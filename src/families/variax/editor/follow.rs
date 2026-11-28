use super::{EditorDocument, FamilyTab, VariaxEditor, egui, live_attachment::Attachment};
use crate::editor::{
    detection::DeviceKey,
    tabs::{DeviceEvent, DeviceTab},
};
use line6_devices::family::variax::{FollowUpdate, Preset};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const CONTROLLER_SETTLE: Duration = Duration::from_millis(250);
const SLOT_SETTLE: Duration = Duration::from_millis(300);
const EDITED_SLOT_SETTLE: Duration = Duration::from_secs(1);
const CLIP_PULSE: Duration = Duration::from_millis(200);
const RESUME_DELAY: Duration = Duration::from_millis(100);
const RIG_RETRY: Duration = Duration::from_millis(150);
const PAUSE_NOTICE_DELAY: Duration = Duration::from_millis(350);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionNotice {
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, Default)]
struct Snapshot {
    program: Option<(Preset, Instant)>,
    volume: Option<u8>,
    tone: Option<u8>,
    tuning_override: Option<bool>,
    clip_until: Option<Instant>,
    dsp_overload: bool,
    queue_overflow: bool,
    notice: Option<ConnectionNotice>,
}

impl Snapshot {
    fn receive(&mut self, update: FollowUpdate, now: Instant) {
        let accept_controller = self
            .program
            .is_none_or(|(_, changed)| now.saturating_duration_since(changed) > CONTROLLER_SETTLE);
        match update {
            FollowUpdate::ProgramChange(preset) => {
                self.program = Some((preset, now));
                self.tone = None;
            }
            FollowUpdate::Volume(value) if accept_controller => self.volume = Some(value),
            FollowUpdate::TonePosition(value) if accept_controller => self.tone = Some(value),
            FollowUpdate::Volume(_) | FollowUpdate::TonePosition(_) => {}
            FollowUpdate::TuningOverride(enabled) => self.tuning_override = Some(enabled),
            FollowUpdate::Clip => self.clip_until = Some(now + CLIP_PULSE),
            FollowUpdate::DspOverload => self.dsp_overload = true,
            FollowUpdate::MidiQueueOverflow => self.queue_overflow = true,
            FollowUpdate::Connected => {
                // Retain a queued disconnect until the shell can invalidate its identity.
                self.notice.get_or_insert(ConnectionNotice::Connected);
            }
            FollowUpdate::Disconnected => self.notice = Some(ConnectionNotice::Disconnected),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ListenEnd {
    Busy,
    Stopped,
    Yielded,
}

type Source = fn(
    &Attachment,
    &AtomicBool,
    &AtomicBool,
    &mut dyn FnMut(),
    &mut dyn FnMut(FollowUpdate, Instant),
) -> Result<ListenEnd, String>;

#[derive(Clone, Copy)]
enum ListenProgress {
    Starting,
    Waiting(Instant),
    Listening,
}

struct WorkerProgress {
    snapshot: Snapshot,
    state: ListenProgress,
    slot_program: Option<(Preset, Instant)>,
}

/// A selector event from the listener handing the rig to foreground work.
pub(super) struct ObservedSlot {
    pub(super) preset: Preset,
    changed: Instant,
    stop: Arc<AtomicBool>,
    continuous: Arc<AtomicBool>,
    progress: Arc<Mutex<WorkerProgress>>,
}

impl ObservedSlot {
    #[cfg(test)]
    pub(super) fn fake(preset: Preset) -> Self {
        let changed = Instant::now();
        Self {
            preset,
            changed,
            stop: Arc::new(AtomicBool::new(false)),
            continuous: Arc::new(AtomicBool::new(true)),
            progress: Arc::new(Mutex::new(WorkerProgress {
                snapshot: Snapshot::default(),
                state: ListenProgress::Listening,
                slot_program: Some((preset, changed)),
            })),
        }
    }

    #[cfg(test)]
    pub(super) fn interrupt(&self) {
        self.continuous.store(false, Ordering::Release);
    }

    pub(super) fn still_valid(&self) -> bool {
        self.continuous.load(Ordering::Acquire)
            && !self.stop.load(Ordering::Relaxed)
            && self
                .progress
                .lock()
                .is_ok_and(|progress| progress.slot_program == Some((self.preset, self.changed)))
    }
}

struct Worker {
    attachment: Attachment,
    stop: Arc<AtomicBool>,
    continuous: Arc<AtomicBool>,
    retry_allowed: Arc<AtomicBool>,
    progress: Arc<Mutex<WorkerProgress>>,
    thread: JoinHandle<Result<ListenEnd, String>>,
}

#[derive(Default)]
pub(super) struct Follow {
    device_key: Option<DeviceKey>,
    attachment: Option<Attachment>,
    worker: Option<Worker>,
    opted_out: bool,
    resume: Option<Instant>,
    paused_since: Option<Instant>,
    #[cfg(test)]
    test_source: Option<Source>,
    snapshot: Snapshot,
    consumed_program: Option<(Preset, Instant)>,
    device_event: Option<DeviceEvent>,
    notice: Option<ConnectionNotice>,
    error: Option<String>,
}

impl Follow {
    #[cfg(test)]
    pub(super) fn test_program(&mut self, preset: Preset, at: Instant) {
        self.snapshot
            .receive(FollowUpdate::ProgramChange(preset), at);
    }

    #[cfg(test)]
    pub(super) fn listen_for_test_slot(&mut self, ctx: &egui::Context) {
        self.start(
            |_, stop, _, ready, publish| {
                ready();
                publish(
                    FollowUpdate::ProgramChange(Preset::new(0, 0).unwrap()),
                    Instant::now(),
                );
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(ListenEnd::Stopped)
            },
            ctx,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self
            .observed_slot(self.attachment.as_ref().unwrap())
            .is_none()
        {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[cfg(test)]
    pub(super) fn queue_disconnect(&mut self, ctx: &egui::Context) {
        self.start(
            |_, _, _, _, publish| {
                publish(FollowUpdate::Disconnected, Instant::now());
                Ok(ListenEnd::Stopped)
            },
            ctx,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !self.worker.as_ref().unwrap().thread.is_finished() {
            assert!(
                Instant::now() < deadline,
                "fake Follow worker did not finish"
            );
            std::thread::yield_now();
        }
    }

    pub(super) fn observe(&mut self, device: Option<&DeviceTab>) {
        let device_key = device.map(|device| device.device.key.clone());
        let attachment = device.and_then(Attachment::verified);
        if self.device_key != device_key || self.attachment != attachment {
            self.stop();
            // The shell can invalidate the attachment before the disconnected
            // worker finishes releasing it. Keep that tab's diagnostic visible.
            if self.device_key != device_key || attachment.is_some() || !self.disconnected() {
                self.error = None;
            }
            if device_key.is_some() && (self.device_key != device_key || attachment.is_some()) {
                self.notice = None;
            }
            self.device_key = device_key;
            self.attachment = attachment;
        }
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn tuning_label(&self) -> Option<&'static str> {
        self.snapshot.tuning_override.map(|enabled| {
            if enabled {
                "Guitar: alternate tuning on"
            } else {
                "Guitar: alternate tuning off"
            }
        })
    }

    pub(super) fn active(&self) -> bool {
        // Keep the surrounding connection and tuning layout stable while
        // a requested Follow session gives up or reacquires the rig.
        self.wants_follow()
    }

    pub(super) fn observed_slot(&self, attachment: &Attachment) -> Option<ObservedSlot> {
        let worker = self.worker.as_ref()?;
        if &worker.attachment != attachment || worker.thread.is_finished() || self.disconnected() {
            return None;
        }
        let (preset, changed) = worker.progress.lock().ok()?.slot_program?;
        let observed = ObservedSlot {
            preset,
            changed,
            stop: worker.stop.clone(),
            continuous: worker.continuous.clone(),
            progress: worker.progress.clone(),
        };
        observed.still_valid().then_some(observed)
    }

    pub(super) fn take_device_event(&mut self) -> Option<DeviceEvent> {
        self.device_event.take()
    }

    pub(super) fn disconnected(&self) -> bool {
        self.notice == Some(ConnectionNotice::Disconnected)
    }

    fn source(&self) -> Option<Source> {
        #[cfg(all(target_os = "linux", not(test)))]
        {
            Some(read_updates)
        }
        #[cfg(test)]
        {
            self.test_source
        }
        #[cfg(all(not(target_os = "linux"), not(test)))]
        {
            None
        }
    }

    fn can_follow(&self) -> bool {
        self.attachment.is_some() && !self.disconnected() && self.source().is_some()
    }

    fn wants_follow(&self) -> bool {
        self.can_follow() && !self.opted_out && self.error.is_none()
    }

    fn set_following(&mut self, following: bool) {
        self.opted_out = !following;
        if following {
            self.error = None;
        } else {
            self.stop();
        }
    }

    pub(super) fn stop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
            worker.thread.thread().unpark();
        }
        self.resume = None;
        self.paused_since = None;
        self.snapshot = Snapshot::default();
        self.consumed_program = None;
        if self.notice == Some(ConnectionNotice::Connected) {
            self.notice = None;
        }
    }

    fn start(&mut self, source: Source, ctx: &egui::Context) -> Result<(), String> {
        if self.busy() {
            return Ok(());
        }
        let attachment = self
            .attachment
            .clone()
            .ok_or("Follow needs a verified Variax attachment.")?;
        if self.disconnected() {
            return Err("Follow needs the guitar connection to be verified again.".into());
        }
        let worker_attachment = attachment.clone();
        let worker_ctx = ctx.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let continuous = Arc::new(AtomicBool::new(false));
        let retry_allowed = Arc::new(AtomicBool::new(true));
        let progress = Arc::new(Mutex::new(WorkerProgress {
            snapshot: self.snapshot,
            state: ListenProgress::Starting,
            slot_program: None,
        }));
        let worker_stop = stop.clone();
        let worker_continuous = continuous.clone();
        let worker_retry_allowed = retry_allowed.clone();
        let worker_progress = progress.clone();
        let thread = std::thread::Builder::new()
            .name("variax-follow".into())
            .spawn(move || {
                let mut ready = || {
                    worker_continuous.store(true, Ordering::Release);
                    if let Ok(mut state) = worker_progress.lock() {
                        state.state = ListenProgress::Listening;
                    }
                    worker_ctx.request_repaint();
                };
                let mut publish = |update, at| {
                    if let Ok(mut state) = worker_progress.lock() {
                        state.slot_program = match update {
                            FollowUpdate::ProgramChange(preset) => Some((preset, at)),
                            FollowUpdate::Connected
                            | FollowUpdate::Disconnected
                            | FollowUpdate::MidiQueueOverflow => None,
                            _ => state.slot_program,
                        };
                        state.snapshot.receive(update, at);
                    }
                    worker_ctx.request_repaint();
                };
                let result = loop {
                    if worker_stop.load(Ordering::Relaxed) {
                        break Ok(ListenEnd::Stopped);
                    }
                    if !worker_retry_allowed.load(Ordering::Relaxed) {
                        std::thread::park_timeout(RIG_RETRY);
                        continue;
                    }
                    let result = source(
                        &worker_attachment,
                        &worker_stop,
                        &worker_continuous,
                        &mut ready,
                        &mut publish,
                    );
                    worker_continuous.store(false, Ordering::Release);
                    if result != Ok(ListenEnd::Busy) {
                        break result;
                    }

                    // A busy acquisition has opened no device. Keep one
                    // worker waiting until the foreground releases the rig.
                    if let Ok(mut state) = worker_progress.lock()
                        && !matches!(state.state, ListenProgress::Waiting(_))
                    {
                        state.state = ListenProgress::Waiting(Instant::now());
                        worker_ctx.request_repaint();
                    }
                    std::thread::park_timeout(RIG_RETRY);
                };
                if let Ok(mut state) = worker_progress.lock() {
                    state.slot_program = None;
                }
                worker_ctx.request_repaint();
                result
            })
            .map_err(|error| format!("Could not start the listener: {error}"))?;
        self.error = None;
        self.worker = Some(Worker {
            attachment,
            stop,
            continuous,
            retry_allowed,
            progress,
            thread,
        });
        ctx.request_repaint();
        Ok(())
    }

    fn listen(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.worker else {
            return;
        };
        let stopping = worker.stop.load(Ordering::Relaxed);
        let finished = worker.thread.is_finished();
        let update = worker.progress.lock().ok().map(|mut progress| {
            let notice = progress.snapshot.notice.take();
            (progress.snapshot, notice, progress.state)
        });
        if let Some((snapshot, notice, state)) = update
            && self.attachment.as_ref() == Some(&worker.attachment)
        {
            match notice {
                Some(ConnectionNotice::Disconnected) => {
                    self.device_event = Some(DeviceEvent::GuitarDisconnected(
                        worker.attachment.key.clone(),
                    ));
                    self.notice = notice;
                    self.stop();
                }
                _ if !stopping => {
                    if notice.is_some() {
                        self.notice = notice;
                    }
                    self.snapshot = snapshot;
                    match state {
                        ListenProgress::Listening => self.paused_since = None,
                        ListenProgress::Waiting(since) => {
                            self.paused_since.get_or_insert(since);
                        }
                        ListenProgress::Starting => {}
                    }
                }
                _ => {}
            }
        }

        if finished {
            let worker = self.worker.take().expect("finished Follow worker");
            let result = worker
                .thread
                .join()
                .unwrap_or_else(|_| Err("The Variax Follow worker stopped unexpectedly.".into()));
            let cancelled = worker.stop.load(Ordering::Relaxed);
            if self.attachment.as_ref() != Some(&worker.attachment) {
                if self.attachment.is_none()
                    && self.disconnected()
                    && self.device_key.as_ref() == Some(&worker.attachment.key)
                    && let Err(error) = result
                {
                    self.error = Some(error);
                }
                return;
            }
            match result {
                Ok(ListenEnd::Yielded) if !cancelled => {
                    let now = Instant::now();
                    self.paused_since.get_or_insert(now);
                    self.resume = Some(now + RESUME_DELAY);
                    // Retain the display, but do not apply a queued selector
                    // from before the foreground operation to the document.
                    self.consumed_program = self.snapshot.program;
                    ctx.request_repaint_after(RESUME_DELAY);
                }
                Ok(ListenEnd::Stopped) if !cancelled => {
                    self.stop();
                    self.error = Some("The listener ended. Turn Follow on to try again.".into());
                }
                Ok(_) => {}
                Err(error) => {
                    self.stop();
                    self.error = Some(error);
                }
            }
        } else {
            // Also repaint on reap if the last worker repaint arrived just
            // before the thread's result became joinable.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn allow_retry(&self, allowed: bool) {
        if let Some(worker) = &self.worker
            && !worker.retry_allowed.swap(allowed, Ordering::Relaxed)
            && allowed
        {
            worker.thread.thread().unpark();
        }
    }

    fn start_if_ready(&mut self, ctx: &egui::Context, now: Instant) {
        if self.busy() || !self.wants_follow() {
            return;
        }
        if let Some(after) = self.resume
            && now < after
        {
            ctx.request_repaint_after(after - now);
            return;
        }
        self.resume = None;
        let source = self.source().expect("available Follow source");
        if let Err(error) = self.start(source, ctx) {
            self.error = Some(error);
        }
    }

    pub(super) fn pending_selection(&self, document: &EditorDocument) -> bool {
        self.snapshot.program.is_some_and(|(preset, at)| {
            self.consumed_program != Some((preset, at))
                && document.selection().is_some_and(|position| {
                    position != (usize::from(preset.bank()), usize::from(preset.slot()))
                })
                && Self::matches_layout(document)
        })
    }

    fn matches_layout(document: &EditorDocument) -> bool {
        document.extension() == "vwb"
            && document.banks().is_some_and(|banks| {
                banks.len() == 12 && banks.iter().all(|bank| bank.presets().len() == 5)
            })
    }

    fn take_settled_slot(
        &mut self,
        document: &EditorDocument,
        now: Instant,
        ctx: &egui::Context,
    ) -> Option<(usize, usize)> {
        let program = self.snapshot.program?;
        if self.consumed_program == Some(program) {
            return None;
        }
        let (preset, changed) = program;
        let bank = usize::from(preset.bank());
        let slot = usize::from(preset.slot());
        if !Self::matches_layout(document) || document.selection() == Some((bank, slot)) {
            self.consumed_program = Some(program);
            return None;
        }
        let delay = if document.slot_is_unsent(bank, slot) {
            EDITED_SLOT_SETTLE
        } else {
            SLOT_SETTLE
        };
        let wait = (changed + delay).saturating_duration_since(now);
        if !wait.is_zero() {
            ctx.request_repaint_after(wait);
            return None;
        }
        self.consumed_program = Some(program);
        Some((bank, slot))
    }

    fn starting(&self) -> bool {
        self.worker.as_ref().is_some_and(|worker| {
            worker
                .progress
                .lock()
                .is_ok_and(|progress| matches!(progress.state, ListenProgress::Starting))
        })
    }

    fn show(&mut self, ui: &mut egui::Ui, available: bool, now: Instant) {
        if self.device_key.is_none() {
            return;
        }
        let stopping = self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.stop.load(Ordering::Relaxed));
        let mut following = self.active();
        ui.horizontal(|ui| {
            let enabled = following || (!stopping && available && self.can_follow());
            if ui
                .add_enabled(
                    enabled,
                    egui::Checkbox::new(&mut following, "Follow selector"),
                )
                .on_hover_text("Show the model selected on the guitar.")
                .on_disabled_hover_text("Show the model selected on the guitar.")
                .changed()
            {
                self.set_following(following);
                ui.ctx().request_repaint();
            }

            let (message, color) = if let Some(error) = &self.error {
                (
                    format!("Follow failed. {error}"),
                    Some(egui::Color32::LIGHT_RED),
                )
            } else if self.disconnected() {
                ("Follow stopped. Guitar disconnected.".into(), None)
            } else if stopping {
                ("Stopping Follow…".into(), None)
            } else if following {
                let paused_since = if !available && !self.busy() {
                    Some(*self.paused_since.get_or_insert(now))
                } else {
                    self.paused_since
                };
                let pause = paused_since.is_some_and(|since| {
                    let remaining = (since + PAUSE_NOTICE_DELAY).saturating_duration_since(now);
                    if !remaining.is_zero() {
                        ui.ctx().request_repaint_after(remaining);
                    }
                    remaining.is_zero()
                });
                self.live_status(ui.ctx(), now, pause, paused_since.is_some())
            } else {
                return;
            };

            // Keep the toolbar at one line at every width. egui exposes
            // the full text on hover when it has to truncate the label.
            let mut label = egui::RichText::new(&message);
            if let Some(color) = color {
                label = label.color(color);
            }
            let label = egui::Label::new(label).truncate();
            if self.error.is_some() {
                let response = ui
                    .add(
                        label
                            .selectable(false)
                            .sense(egui::Sense::click())
                            .show_tooltip_when_elided(false),
                    )
                    .on_hover_text(&message)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                egui::Popup::menu(&response).show(|ui| {
                    ui.set_max_width(420.0);
                    ui.add(egui::Label::new(&message).wrap().selectable(true));
                    if ui.button("Copy Follow error").clicked() {
                        ui.ctx().copy_text(message);
                    }
                });
            } else {
                ui.add(label);
            }
        });
    }

    fn live_status(
        &self,
        ctx: &egui::Context,
        now: Instant,
        paused: bool,
        waiting_for_rig: bool,
    ) -> (String, Option<egui::Color32>) {
        let snapshot = self.snapshot;
        let mut fields = vec![
            control_position("Observed volume", snapshot.volume),
            control_position("Observed tone", snapshot.tone),
        ];
        if let Some((preset, _)) = snapshot.program {
            fields.push(format!(
                "Guitar preset · {} {}",
                preset.bank_name(),
                preset.slot() + 1
            ));
        }
        if paused {
            fields.push("Follow paused while the rig is in use.".into());
        } else if waiting_for_rig {
            fields.push("Waiting for the rig".into());
        } else if self.starting() {
            fields.push("Starting the guitar listener…".into());
        } else if snapshot.program.is_none() {
            fields.push("Waiting for a guitar preset change".into());
        }
        if self.notice == Some(ConnectionNotice::Connected) {
            fields.push("Guitar connected via PODxt Live.".into());
        }
        let mut color = None;
        if let Some(until) = snapshot.clip_until.filter(|until| *until > now) {
            fields.push("Clip".into());
            color = Some(super::FRET_MARKER);
            ctx.request_repaint_after(until - now);
        }
        if snapshot.queue_overflow {
            fields.push("MIDI queue overflow".into());
            color = Some(super::FRET_MARKER);
        }
        if snapshot.dsp_overload {
            fields.push("DSP overload".into());
            color = Some(egui::Color32::LIGHT_RED);
        }
        (fields.join("  ·  "), color)
    }
}

impl Drop for Follow {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.thread.join();
        }
    }
}

fn control_position(label: &str, value: Option<u8>) -> String {
    match value {
        Some(value) => format!("{label} {}%", (u16::from(value) * 100 + 63) / 127),
        None => format!("{label} —"),
    }
}

impl VariaxEditor {
    pub(super) fn follow_panel(&mut self, ui: &mut egui::Ui) {
        let available = self.follow_available();
        self.follow.show(ui, available, Instant::now());
    }

    fn follow_available(&self) -> bool {
        !self.device_sync.stopped_after_store()
            && !self.is_reading()
            && !self.blocking_prompt()
            && !self.live_edit.busy()
    }

    pub(super) fn observe_follow(&mut self, ctx: &egui::Context) {
        if self.device_sync.stopped_after_store() {
            self.follow.set_following(false);
        }
        self.follow.listen(ctx);
        if self.follow.disconnected() {
            self.device_sync.dismiss_prompt();
        }
    }

    pub(super) fn advance_follow(&mut self, ctx: &egui::Context) {
        let available = self.follow_available();
        self.follow.allow_retry(available);
        if available {
            self.follow.start_if_ready(ctx, Instant::now());
        }
        if self.is_reading()
            || self.has_historical_guitar_library()
            || self.live_edit.busy()
            || self.blocking_prompt()
            || self.name_draft.is_some()
            || self.author_draft.is_some()
            || ctx.input(|input| input.pointer.any_down())
        {
            return;
        }
        if let Some(document) = &mut self.document
            && let Some((bank, slot)) = self.follow.take_settled_slot(document, Instant::now(), ctx)
        {
            if let Err(error) = document.select(bank, slot) {
                self.follow.error = Some(error);
            } else {
                self.drag_origin = None;
                self.clear_text_drafts();
                self.live_selection_followed();
            }
        }
    }
}

#[cfg(any(target_os = "linux", test))]
struct ListenerLog<'a> {
    ready: &'a mut dyn FnMut(),
    listening: bool,
}

#[cfg(any(target_os = "linux", test))]
impl line6_devices::session::Log for ListenerLog<'_> {
    fn line(&mut self, line: String) {
        // The family emits this after identity, bridge enable and session on.
        // Keep the readiness marker covered by a fake family-session test.
        if line.ends_with(" Listening window open for Variax Follow.") {
            self.listening = true;
            (self.ready)();
        }
    }
}

#[cfg(any(target_os = "linux", test))]
fn report_session(
    result: Result<(), String>,
    listening: bool,
    packets: usize,
    started_setup: bool,
    session_cleanup_failed: bool,
    usb_release: Result<(), String>,
) -> Result<(), String> {
    let session_error = result.err().map(|error| {
        let release_only = session_cleanup_failed
            && (error.starts_with("Session off failed:")
                || error.starts_with("PODxt Variax-messages release failed:"));
        let stage = if release_only {
            "Session or relay release"
        } else if listening {
            "Listener read"
        } else if started_setup || session_cleanup_failed {
            "Listener setup"
        } else if error == "No reply to the identity request: nothing arrived from the guitar." {
            return if packets == 0 {
                format!("Identity silence: {error}")
            } else {
                format!("Identity: received {packets} packets without a valid reply. {error}")
            };
        } else {
            "Identity"
        };
        if session_cleanup_failed && !release_only {
            format!("{stage}: {error}\nSession or relay release also failed.")
        } else {
            format!("{stage}: {error}")
        }
    });
    let usb_error = usb_release
        .err()
        .map(|error| format!("USB release (interface 1): {error}"));
    match (session_error, usb_error) {
        (Some(error), Some(release)) => Err(format!("{error}\n{release}")),
        (Some(error), None) | (None, Some(error)) => Err(error),
        (None, None) => Ok(()),
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn read_updates(
    attachment: &Attachment,
    stop: &AtomicBool,
    continuous: &AtomicBool,
    ready: &mut dyn FnMut(),
    publish: &mut dyn FnMut(FollowUpdate, Instant),
) -> Result<ListenEnd, String> {
    use line6_devices::{
        device_lock::{FollowDeviceLock, LockError},
        family::variax::follow_reporting_cleanup,
        gate::{GatePolicy, Operation},
        session::Session,
    };
    use std::cell::Cell;
    use variax_workbench::hardware_gates::{Gate, is_locked, run_if_unlocked};

    struct Policy;
    impl GatePolicy for Policy {
        fn allows(&self, operation: Operation) -> bool {
            match operation {
                Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
                Operation::VariaxRead => !is_locked(Gate::CurrentSettingsRead),
                _ => false,
            }
        }
    }

    run_if_unlocked(Gate::OsDiscovery, || Ok(()))?;
    run_if_unlocked(Gate::IdentityRequest, || Ok(()))?;
    run_if_unlocked(Gate::CurrentSettingsRead, || {
        if stop.load(Ordering::Relaxed) {
            return Ok(ListenEnd::Stopped);
        }
        let lease = match FollowDeviceLock::acquire() {
            Ok(lease) => lease,
            Err(LockError::Busy | LockError::BusyHere) => return Ok(ListenEnd::Busy),
            Err(error) => return Err(format!("Rig lock: {error}")),
        };
        let yielded = Cell::new(false);
        let cancelled = || {
            if !yielded.get() && lease.yield_requested() {
                // Release slot authority before closing the listener or handing over the lock.
                continuous.store(false, Ordering::Release);
                yielded.set(true);
            }
            yielded.get() || stop.load(Ordering::Relaxed)
        };
        if cancelled() {
            return Ok(if yielded.get() {
                ListenEnd::Yielded
            } else {
                ListenEnd::Stopped
            });
        }
        let cleanup_uncertain = Cell::new(true);
        let result = (|| {
            let link = variax_workbench::usb::open_variax_bridge_borrowed(
                lease.rig(),
                (&attachment.path, attachment.address),
            )
            .map_err(|error| format!("USB open (interface 1): {error}"))?;
            cleanup_uncertain.set(false);
            let mut log = ListenerLog {
                ready,
                listening: false,
            };
            let mut session = Session::new(link, &Policy, &mut log);
            session.log_packets = false;
            let result = follow_reporting_cleanup(
                lease.rig(),
                &mut session,
                cancelled,
                |update| publish(update, Instant::now()),
                || cleanup_uncertain.set(true),
            );
            let packets = session.packets_in;
            // Any successful send after the identity exchange belongs to
            // listener setup or release. This includes a successful relay-off
            // after a failed relay-on.
            let started_setup = session
                .sent
                .iter()
                .any(|packet| packet.as_slice() != line6_devices::identity::REQUEST);
            let release = session.link.finish();
            let session_cleanup_failed = cleanup_uncertain.get();
            if release.is_err() {
                cleanup_uncertain.set(true);
            }
            report_session(
                result,
                log.listening,
                packets,
                started_setup,
                session_cleanup_failed,
                release,
            )
        })();
        continuous.store(false, Ordering::Release);
        match result {
            Ok(()) if yielded.get() => Ok(ListenEnd::Yielded),
            Ok(()) => Ok(ListenEnd::Stopped),
            Err(error) => {
                if cleanup_uncertain.get() && (yielded.get() || lease.yield_requested()) {
                    lease.cleanup_failed();
                }
                Err(error)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::refresh::tests as refresh_tests;
    use super::*;
    use crate::editor::registry::DEVICE_PROFILES;
    use crate::editor::tests::app;
    use variax_workbench::{bundle::BundleFile, edit::WIRING};

    #[test]
    fn fake_family_session_distinguishes_identity_silence_setup_and_listener_read() {
        use line6_devices::{
            family::variax::{VariaxRequest, follow_window},
            gate::{GatePolicy, Operation},
            session::{Link, Path, Request, Session},
        };

        struct ReadPolicy;
        impl GatePolicy for ReadPolicy {
            fn allows(&self, operation: Operation) -> bool {
                matches!(operation, Operation::VariaxIdentity | Operation::VariaxRead)
            }
        }

        enum Scenario {
            Silent,
            UnrelatedPacket,
            SetupError,
            ReadError,
        }

        struct FakeLink {
            scenario: Scenario,
            elapsed: Duration,
            writes: Vec<Vec<u8>>,
            reads: usize,
        }

        impl Link for FakeLink {
            fn path(&self) -> Path {
                Path::VariaxViaPodxt
            }

            fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
                self.writes.push(bytes.to_vec());
                if matches!(self.scenario, Scenario::SetupError)
                    && bytes == VariaxRequest::SessionOn.bytes()
                {
                    return Err("fake session-on failure".into());
                }
                Ok(())
            }

            fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
                self.elapsed += wait;
                self.reads += 1;
                match (&self.scenario, self.reads) {
                    (Scenario::Silent, _) => Ok(None),
                    (Scenario::UnrelatedPacket, 1) => Ok(Some(vec![0xb0, 7, 10])),
                    (Scenario::UnrelatedPacket, _) => Ok(None),
                    (_, 1) => Ok(Some(vec![
                        0xf0, 0x7e, 0x7f, 0x06, 0x02, 0, 1, 0x0c, 7, 0, 4, 0, b'0', b'3', b'1',
                        b'0', 0xf7,
                    ])),
                    (Scenario::ReadError, _) => Err("fake interrupt transfer failure".into()),
                    (Scenario::SetupError, _) => panic!("setup failed before listening"),
                }
            }

            fn elapsed(&self) -> Duration {
                self.elapsed
            }

            fn pause(&mut self, duration: Duration) {
                self.elapsed += duration;
            }
        }

        for (scenario, expected, expected_writes, ready_expected) in [
            (Scenario::Silent, "Identity silence:", 1, false),
            (Scenario::UnrelatedPacket, "Identity:", 1, false),
            (
                Scenario::SetupError,
                "Listener setup: fake session-on failure",
                5,
                false,
            ),
            (
                Scenario::ReadError,
                "Listener read: fake interrupt transfer failure",
                5,
                true,
            ),
        ] {
            let mut ready = false;
            let mut mark_ready = || ready = true;
            let mut log = ListenerLog {
                ready: &mut mark_ready,
                listening: false,
            };
            let link = FakeLink {
                scenario,
                elapsed: Duration::ZERO,
                writes: Vec::new(),
                reads: 0,
            };
            let mut session = Session::new(link, &ReadPolicy, &mut log);
            session.log_packets = false;
            let result = follow_window(
                &mut session,
                Duration::from_secs(1),
                false,
                || false,
                |_| {
                    panic!("no Follow updates expected");
                },
            );
            let packets = session.packets_in;
            let started_setup = session
                .sent
                .iter()
                .any(|packet| packet.as_slice() != line6_devices::identity::REQUEST);
            assert_eq!(session.link.writes.len(), expected_writes);
            let error =
                report_session(result, log.listening, packets, started_setup, false, Ok(()))
                    .unwrap_err();
            assert!(error.starts_with(expected), "{error}");
            assert_eq!(ready, ready_expected);
        }
    }

    #[test]
    fn follow_keeps_listener_and_cleanup_errors_in_the_same_diagnostic() {
        let error = report_session(
            Err("interrupt read failed. Session off also failed: I/O".into()),
            true,
            1,
            true,
            true,
            Err("release interface: busy. reattach kernel driver: I/O".into()),
        )
        .unwrap_err();
        assert!(error.starts_with("Listener read: interrupt read failed."));
        assert!(error.contains("Session off also failed: I/O"));
        assert!(error.contains(
            "USB release (interface 1): release interface: busy. reattach kernel driver: I/O"
        ));
        assert_eq!(
            report_session(
                Err("Session off failed: I/O".into()),
                true,
                1,
                true,
                true,
                Ok(())
            ),
            Err("Session or relay release: Session off failed: I/O".into()),
        );
        assert_eq!(
            report_session(
                Ok(()),
                true,
                1,
                true,
                false,
                Err("reattach kernel driver: I/O".into())
            ),
            Err("USB release (interface 1): reattach kernel driver: I/O".into()),
        );
    }

    fn fake_source(
        attachment: &Attachment,
        stop: &AtomicBool,
        _: &AtomicBool,
        ready: &mut dyn FnMut(),
        publish: &mut dyn FnMut(FollowUpdate, Instant),
    ) -> Result<ListenEnd, String> {
        assert_eq!(
            (attachment.path.as_str(), attachment.address),
            ("8-2.1", 10)
        );
        ready();
        let now = Instant::now();
        let changed = now - Duration::from_secs(2);
        let preset = Preset::new(4, 2).unwrap();
        publish(FollowUpdate::Connected, changed);
        publish(FollowUpdate::ProgramChange(preset), changed);
        publish(
            FollowUpdate::Volume(1),
            changed + Duration::from_millis(249),
        );
        publish(FollowUpdate::TonePosition(2), changed + CONTROLLER_SETTLE);
        publish(
            FollowUpdate::Volume(64),
            changed + Duration::from_millis(251),
        );
        publish(FollowUpdate::TonePosition(127), now);
        publish(FollowUpdate::Clip, now);
        publish(FollowUpdate::DspOverload, now);
        publish(FollowUpdate::MidiQueueOverflow, now);
        while !stop.load(Ordering::Relaxed) && now.elapsed() < Duration::from_secs(2) {
            std::thread::yield_now();
        }
        Ok(ListenEnd::Stopped)
    }

    fn poll_follow(editor: &mut VariaxEditor, ctx: &egui::Context) {
        editor.observe_follow(ctx);
        editor.advance_follow(ctx);
    }

    fn poll_until(
        editor: &mut VariaxEditor,
        ctx: &egui::Context,
        predicate: impl Fn(&Follow) -> bool,
    ) {
        let started = Instant::now();
        while !predicate(&editor.follow) {
            poll_follow(editor, ctx);
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
    }

    #[test]
    fn follow_starts_only_after_verification_and_checkbox_preserves_opt_out() {
        use crate::editor::detection::DeviceSupport;

        fn checkbox_center(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == "Follow selector" => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(checkbox_center),
                _ => None,
            }
        }

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.follow.test_source = Some(fake_source);
        poll_follow(editor, &ctx);
        assert!(!editor.follow.busy());

        let mut tab = super::super::refresh::tests::verified_tab();
        let verified = std::mem::replace(&mut tab.device.support, DeviceSupport::IdentityPending);
        editor.observe_device(Some(&tab));
        poll_follow(editor, &ctx);
        assert!(!editor.follow.busy());

        tab.device.support = verified;
        editor.observe_device(Some(&tab));
        poll_until(editor, &ctx, |follow| follow.snapshot.volume == Some(64));

        let frame = |editor: &mut VariaxEditor, events| {
            ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| editor.follow_panel(ui));
                },
            )
        };
        let center = frame(editor, vec![])
            .shapes
            .iter()
            .find_map(|item| checkbox_center(&item.shape))
            .unwrap();
        let click = |editor: &mut VariaxEditor| {
            let _ = frame(editor, vec![egui::Event::PointerMoved(center)]);
            for pressed in [true, false] {
                let _ = frame(
                    editor,
                    vec![egui::Event::PointerButton {
                        pos: center,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
        };

        click(editor);
        poll_until(editor, &ctx, |follow| !follow.busy());
        assert!(!editor.follow.wants_follow());
        editor.observe_device(None);
        editor.observe_device(Some(&tab));
        poll_follow(editor, &ctx);
        assert!(!editor.follow.busy());

        click(editor);
        poll_until(editor, &ctx, |follow| follow.snapshot.volume == Some(64));
        editor.observe_device(None);
        poll_until(editor, &ctx, |follow| !follow.busy());
        editor.observe_device(Some(&tab));
        poll_until(editor, &ctx, |follow| follow.snapshot.volume == Some(64));
    }

    #[test]
    fn verified_follow_reads_live_values_without_editing_the_document() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        assert!(editor.follow.start(fake_source, &ctx).is_err());
        refresh_tests::attach(editor);
        refresh_tests::download(editor, &ctx);

        let document = editor.document.as_mut().unwrap();
        document.select(4, 2).unwrap();
        document
            .apply(|preset| preset.set_parameter(WIRING, 0x200005, 0, 1))
            .unwrap();
        let before = document.as_bytes().to_vec();
        document.select(0, 0).unwrap();
        editor.follow.start(fake_source, &ctx).unwrap();
        poll_until(editor, &ctx, |follow| follow.snapshot.queue_overflow);
        let snapshot = editor.follow.snapshot;
        assert_eq!(editor.follow.notice, Some(ConnectionNotice::Connected));
        assert!(editor.follow.busy());
        assert_eq!(snapshot.program.unwrap().0, Preset::new(4, 2).unwrap());
        assert_eq!(snapshot.volume, Some(64));
        assert_eq!(snapshot.tone, Some(127));
        assert!(snapshot.clip_until.is_some() && snapshot.dsp_overload);
        assert_eq!(
            control_position("Observed tone", snapshot.tone),
            "Observed tone 100%"
        );
        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((4, 2)));
        assert_eq!(
            document.preset().parameter(WIRING, 0x200005).unwrap().bits,
            1
        );
        assert_eq!(document.as_bytes(), before);
        assert!(document.can_undo());

        editor.follow.stop();
        poll_until(editor, &ctx, |follow| !follow.busy());
        assert!(editor.follow.snapshot.program.is_none());

        editor.follow.start(fake_source, &ctx).unwrap();
        editor.observe_device(None);
        poll_until(editor, &ctx, |follow| !follow.busy());
        assert!(editor.follow.attachment.is_none());
        super::super::refresh::tests::attach(editor);
        assert!(editor.follow.attachment.is_some());
        assert!(!editor.follow.busy());
    }

    #[test]
    fn follow_keeps_the_selection_in_a_historical_library() {
        use super::super::LibraryOrigin;
        use std::time::SystemTime;

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        let tab = super::super::refresh::tests::verified_tab();
        editor.observe_device(Some(&tab));
        editor
            .adopt_startup_library(
                BundleFile::new_default_bundle().unwrap(),
                LibraryOrigin::Download(SystemTime::UNIX_EPOCH),
            )
            .unwrap();
        editor.follow.snapshot.receive(
            FollowUpdate::ProgramChange(Preset::new(4, 2).unwrap()),
            Instant::now() - SLOT_SETTLE,
        );
        let selected = editor.document.as_ref().unwrap().selection();
        assert_ne!(selected, Some((4, 2)));
        poll_follow(editor, &ctx);
        assert_eq!(editor.document.as_ref().unwrap().selection(), selected);

        editor.library_origin = Some(LibraryOrigin::AttachedDownload(
            SystemTime::UNIX_EPOCH,
            tab.device.key,
        ));
        poll_follow(editor, &ctx);
        assert_eq!(editor.document.as_ref().unwrap().selection(), Some((4, 2)));
    }

    #[test]
    fn reverified_refresh_discards_queued_follow_updates_then_follows_the_current_library() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        let mut rig = refresh_tests::verified_rig();
        let first = refresh_tests::verified_tab_for(&mut rig);
        editor.observe_device(Some(&first));
        refresh_tests::download(editor, &ctx);
        editor.change(|preset| preset.rename("Keep this edit"));
        let document = editor.document.as_ref().unwrap();
        let selected = document.selection();
        let before = document.as_bytes().to_vec();
        assert_ne!(selected, Some((4, 2)));
        assert_ne!(selected, Some((5, 1)));

        let old_source: Source = |_, _, _, _, publish| {
            let now = Instant::now();
            publish(
                FollowUpdate::ProgramChange(Preset::new(5, 1).unwrap()),
                now - Duration::from_secs(2),
            );
            publish(FollowUpdate::TonePosition(12), now);
            publish(FollowUpdate::Disconnected, now);
            Ok(ListenEnd::Stopped)
        };
        editor.follow.start(old_source, &ctx).unwrap();
        // Leave the old snapshot queued until after the shell has reverified.
        let started = Instant::now();
        while !editor.follow.worker.as_ref().unwrap().thread.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }

        rig.report_guitar_disconnect(&first.device.key, DEVICE_PROFILES)
            .unwrap();
        editor.observe_device(None);
        assert!(rig.queue_guitar_refresh(&first.device.key.attachment, false));
        let second = refresh_tests::verified_tab_for(&mut rig);
        assert_eq!(first.device.key.attachment, second.device.key.attachment);
        assert_eq!(first.device.support, second.device.support);
        assert_ne!(first.device.key, second.device.key);
        editor.observe_device(Some(&second));
        // The old snapshot and the new attachment's completed Refresh reach
        // the editor in the same poll.
        refresh_tests::download_and_keep(editor, &ctx);
        assert!(!editor.follow.busy());
        assert!(!editor.follow.disconnected());
        assert!(editor.take_device_event().is_none());
        assert!(editor.follow.snapshot.program.is_none());
        assert!(editor.follow.snapshot.tone.is_none());
        assert_eq!(editor.document.as_ref().unwrap().selection(), selected);

        assert!(editor.has_historical_guitar_library());
        assert!(matches!(
            editor.library_origin.as_ref(),
            Some(super::super::LibraryOrigin::AttachedDownload(_, key)) if *key == first.device.key
        ));
        editor.follow.test_source = Some(fake_source);
        let started = Instant::now();
        while !editor.follow.snapshot.queue_overflow {
            editor.poll(&ctx);
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::yield_now();
        }
        assert_eq!(
            editor.follow.worker.as_ref().unwrap().attachment.key,
            second.device.key
        );
        assert_eq!(
            editor.follow.snapshot.program.unwrap().0,
            Preset::new(4, 2).unwrap()
        );
        assert_eq!(editor.follow.snapshot.tone, Some(127));
        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), selected);
        assert_eq!(document.as_bytes(), before);
        assert!(document.is_dirty() && document.can_undo());

        // The retained download is available for explicit opening. Undo the
        // edit first so opening uses the clean-document path.
        editor.undo();
        assert!(!editor.dirty());
        editor.open_download();
        assert!(editor.has_current_guitar_library());
        assert!(matches!(
            editor.library_origin.as_ref(),
            Some(super::super::LibraryOrigin::AttachedDownload(_, key)) if *key == second.device.key
        ));
        assert_eq!(editor.document.as_ref().unwrap().selection(), Some((3, 2)));
        editor.change(|preset| preset.rename("New rig edit"));
        let edited = editor.document.as_ref().unwrap().as_bytes().to_vec();

        editor.poll(&ctx);

        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((4, 2)));
        assert_eq!(document.as_bytes(), edited);
        assert!(document.slot_is_dirty(3, 2) && document.can_undo());
        assert_eq!(editor.follow.snapshot.tone, Some(127));
        assert!(editor.take_device_event().is_none());
    }

    #[test]
    fn the_follow_panel_reopens_the_same_attachment_after_foreground_work() {
        use std::sync::atomic::AtomicUsize;

        static OPEN_COUNT: AtomicUsize = AtomicUsize::new(0);
        static ACQUIRE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
        static DEVICE_CHECK_HOLDS_RIG: AtomicBool = AtomicBool::new(false);

        fn yielding_source(
            attachment: &Attachment,
            stop: &AtomicBool,
            _: &AtomicBool,
            ready: &mut dyn FnMut(),
            publish: &mut dyn FnMut(FollowUpdate, Instant),
        ) -> Result<ListenEnd, String> {
            assert_eq!(
                (attachment.path.as_str(), attachment.address),
                ("8-2.1", 10)
            );
            ACQUIRE_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            if DEVICE_CHECK_HOLDS_RIG.load(Ordering::SeqCst) {
                return Ok(ListenEnd::Busy);
            }
            let now = Instant::now();
            if OPEN_COUNT.fetch_add(1, Ordering::SeqCst) == 0 {
                ready();
                publish(
                    FollowUpdate::ProgramChange(Preset::new(3, 2).unwrap()),
                    now - SLOT_SETTLE,
                );
                publish(FollowUpdate::Volume(35), now);
                publish(FollowUpdate::TonePosition(44), now);
                return Ok(ListenEnd::Yielded);
            }
            ready();
            publish(
                FollowUpdate::ProgramChange(Preset::new(4, 2).unwrap()),
                now - SLOT_SETTLE,
            );
            publish(FollowUpdate::Volume(64), now);
            publish(FollowUpdate::TonePosition(105), now);
            while !stop.load(Ordering::Relaxed) && now.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(ListenEnd::Stopped)
        }

        fn shows_pause(ctx: &egui::Context, follow: &mut Follow) -> bool {
            const PAUSE: &str = "Follow paused while the rig is in use.";
            fn contains(shape: &egui::Shape) -> bool {
                match shape {
                    egui::Shape::Text(text) => text.galley.text().contains(PAUSE),
                    egui::Shape::Vec(shapes) => shapes.iter().any(contains),
                    _ => false,
                }
            }
            ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    follow.show(ui, true, Instant::now() + PAUSE_NOTICE_DELAY)
                });
            })
            .shapes
            .iter()
            .any(|item| contains(&item.shape))
        }

        enum Foreground {
            DeviceCheck,
            LiveEdit,
            Store {
                error: Option<&'static str>,
                unknown: bool,
            },
        }
        for operation in [
            Foreground::DeviceCheck,
            Foreground::LiveEdit,
            Foreground::Store {
                error: None,
                unknown: false,
            },
            Foreground::Store {
                error: Some("Injected pre-commit failure"),
                unknown: false,
            },
            Foreground::Store {
                error: Some("Store outcome unknown"),
                unknown: true,
            },
        ] {
            OPEN_COUNT.store(0, Ordering::SeqCst);
            ACQUIRE_ATTEMPTS.store(0, Ordering::SeqCst);
            let (ctx, mut app) = app();
            let editor = app.variax_mut();
            refresh_tests::attach(editor);
            refresh_tests::download(editor, &ctx);
            let document = editor.document.as_ref().unwrap();
            let selected = document.selection();
            let original = document.as_bytes().to_vec();
            editor.follow.test_source = Some(yielding_source);
            poll_follow(editor, &ctx);
            assert!(editor.follow.busy());
            let started = Instant::now();
            while editor.follow.resume.is_none() {
                editor.follow.listen(&ctx);
                assert!(started.elapsed() < Duration::from_secs(1));
                std::thread::yield_now();
            }
            assert!(shows_pause(&ctx, &mut editor.follow));
            let ready = editor.follow.resume.unwrap();
            editor.follow.start_if_ready(&ctx, ready - RESUME_DELAY);
            assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);

            let (expected_error, unknown, opens_after_resume) = match operation {
                Foreground::DeviceCheck => {
                    use variax_workbench::device_probe::{
                        Consent, ProbeSession, Scenario, SimulatedRig, Stage, StageStatus,
                    };
                    // The fake source refuses the lease until Device Check releases it.
                    DEVICE_CHECK_HOLDS_RIG.store(true, Ordering::SeqCst);
                    editor.follow.resume = Some(Instant::now());
                    poll_follow(editor, &ctx);
                    poll_until(editor, &ctx, |follow| {
                        follow.worker.as_ref().is_some_and(|worker| {
                            matches!(
                                worker.progress.lock().unwrap().state,
                                ListenProgress::Waiting(_)
                            )
                        })
                    });
                    assert!(shows_pause(&ctx, &mut editor.follow));
                    assert_eq!(editor.follow.snapshot.volume, Some(35));
                    assert_eq!(editor.follow.snapshot.tone, Some(44));
                    assert_eq!(editor.document.as_ref().unwrap().selection(), selected);
                    assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);
                    let waiter = editor
                        .follow
                        .worker
                        .as_ref()
                        .unwrap()
                        .thread
                        .thread()
                        .clone();
                    waiter.unpark();
                    let attempts = ACQUIRE_ATTEMPTS.load(Ordering::SeqCst);
                    poll_until(editor, &ctx, |_| {
                        ACQUIRE_ATTEMPTS.load(Ordering::SeqCst) > attempts
                    });
                    assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        editor.follow.worker.as_ref().unwrap().thread.thread().id(),
                        waiter.id()
                    );
                    assert!(shows_pause(&ctx, &mut editor.follow));

                    let mut check = ProbeSession::new(SimulatedRig::new(Scenario::Connected));
                    check.answer(Consent::Yes);
                    assert_eq!(check.status(Stage::UsbRecognition), StageStatus::Passed);
                    DEVICE_CHECK_HOLDS_RIG.store(false, Ordering::SeqCst);
                    waiter.unpark();
                    poll_until(editor, &ctx, |follow| follow.snapshot.volume == Some(64));
                    assert_eq!(
                        editor.follow.worker.as_ref().unwrap().thread.thread().id(),
                        waiter.id()
                    );
                    (None, false, 2)
                }
                Foreground::LiveEdit => {
                    let finish = editor.hold_fake_edit();
                    editor.follow.resume = Some(Instant::now());
                    poll_follow(editor, &ctx);
                    assert!(shows_pause(&ctx, &mut editor.follow));
                    let (status, _) = editor.follow.live_status(&ctx, Instant::now(), true, true);
                    assert!(status.contains("Observed volume 28%"));
                    assert!(status.contains("Observed tone 35%"));
                    assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);
                    finish.send(()).unwrap();
                    let started = Instant::now();
                    while editor.live_edit.busy() {
                        editor.poll_edit_buffer(&ctx);
                        assert!(started.elapsed() < Duration::from_secs(1));
                        std::thread::yield_now();
                    }
                    (None, false, 2)
                }
                Foreground::Store { error, unknown } => {
                    let finish = editor.hold_fake_store(super::super::device_sync::Report {
                        error: error.map(str::to_owned),
                        store_unknown: unknown,
                        ..Default::default()
                    });
                    editor.follow.resume = Some(Instant::now());
                    poll_follow(editor, &ctx);
                    assert!(shows_pause(&ctx, &mut editor.follow));
                    assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);
                    finish.send(()).unwrap();
                    let started = Instant::now();
                    while editor.device_sync.busy() {
                        editor.poll_device_sync(&ctx);
                        assert!(started.elapsed() < Duration::from_secs(1));
                        std::thread::yield_now();
                    }
                    (error, unknown, 2)
                }
            };
            if unknown {
                poll_follow(editor, &ctx);
                assert!(editor.follow.resume.is_none() && !editor.follow.busy());
                assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), 1);
            } else {
                poll_until(editor, &ctx, |follow| follow.snapshot.tone == Some(105));
                assert_eq!(editor.follow.snapshot.volume, Some(64));
                assert!(editor.follow.paused_since.is_none());
                assert_eq!(editor.document.as_ref().unwrap().selection(), Some((4, 2)));
                assert_eq!(OPEN_COUNT.load(Ordering::SeqCst), opens_after_resume);
            }
            assert_eq!(editor.document.as_ref().unwrap().as_bytes(), original);
            assert_eq!(editor.error.as_deref(), expected_error);
        }
    }

    #[test]
    fn follow_status_changes_keep_the_real_editor_workspace_in_place() {
        fn text_center(output: &egui::FullOutput, expected: &str) -> Option<egui::Pos2> {
            fn find(shape: &egui::Shape, expected: &str) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(text) if text.galley.text().contains(expected) => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    egui::Shape::Vec(shapes) => {
                        shapes.iter().find_map(|shape| find(shape, expected))
                    }
                    _ => None,
                }
            }
            output
                .shapes
                .iter()
                .find_map(|item| find(&item.shape, expected))
        }

        let has_text =
            |output: &egui::FullOutput, expected| text_center(output, expected).is_some();

        for (width, density) in [(960.0, 1.0), (1120.0, 2.0)] {
            let (ctx, mut app) = app();
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.clear_window_controls = true;
            let index = app.tabs.index(app.tabs.active.unwrap()).unwrap();
            app.tabs.entries[index].device = Some(refresh_tests::verified_tab());
            let editor = app.variax_mut();
            refresh_tests::attach(editor);
            refresh_tests::download(editor, &ctx);
            editor.follow.test_source = Some(fake_source);

            let now = Instant::now();
            editor.follow.resume = Some(now + Duration::from_secs(10));
            let frame_with_events = |app: &mut crate::editor::Workbench, events| {
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 760.0),
                    )),
                    events,
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(density);
                ctx.run(input, |ctx| app.render(ctx))
            };
            let frame = |app: &mut crate::editor::Workbench| frame_with_events(app, vec![]);
            let click = |app: &mut crate::editor::Workbench, pos| {
                let _ = frame_with_events(app, vec![egui::Event::PointerMoved(pos)]);
                for pressed in [true, false] {
                    let output = frame_with_events(
                        app,
                        vec![egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                    if !pressed {
                        return output;
                    }
                }
                unreachable!()
            };
            let layout = |app: &crate::editor::Workbench| {
                let editor = app.variax();
                (
                    editor.workspace_cards.unwrap(),
                    editor.workspace_tuning.unwrap(),
                )
            };

            let _ = frame(&mut app);
            let waiting = frame(&mut app);
            assert!(has_text(&waiting, "Waiting for a guitar preset change"));
            let original = layout(&app);
            let tuning_widgets = app.variax().tuning_widgets.clone();

            for (time_paused, should_show_pause) in [
                (Some(Duration::ZERO), false),
                (Some(PAUSE_NOTICE_DELAY), true),
                (None, false),
            ] {
                let follow = &mut app.variax_mut().follow;
                follow.paused_since = time_paused.map(|elapsed| Instant::now() - elapsed);
                follow.snapshot.volume = Some(35);
                follow.snapshot.tone = Some(44);
                follow.snapshot.program = Some((Preset::new(3, 2).unwrap(), now - SLOT_SETTLE));
                follow.snapshot.clip_until = Some(now + Duration::from_secs(10));
                let output = frame(&mut app);
                assert!(has_text(&output, "Observed volume 28%"));
                assert!(has_text(&output, "Observed tone 35%"));
                assert!(has_text(&output, "Guitar preset"));
                assert_eq!(
                    has_text(&output, "Waiting for the rig"),
                    time_paused == Some(Duration::ZERO),
                );
                assert_eq!(
                    has_text(&output, "Follow paused while the rig is in use."),
                    should_show_pause,
                );
                assert_eq!(layout(&app), original);
                assert_eq!(app.variax().tuning_widgets, tuning_widgets);
            }

            let error = format!(
                "USB open (interface 1): {}\nComplete driver error.",
                "Injected transport failure ".repeat(20)
            );
            app.variax_mut().follow.error = Some(error.clone());
            let failed = frame(&mut app);
            let status = text_center(&failed, "Follow failed. USB open").unwrap();
            let _ = click(&mut app, status);
            let details = frame(&mut app);
            assert!(has_text(&details, "Complete driver error."));
            let copy = text_center(&details, "Copy Follow error").unwrap();
            let copied = click(&mut app, copy);
            assert!(
                copied
                    .platform_output
                    .commands
                    .contains(&egui::OutputCommand::CopyText(format!(
                        "Follow failed. {error}"
                    )))
            );
            assert_eq!(layout(&app), original);
        }
    }

    #[test]
    fn completed_worker_delivers_disconnect_before_its_snapshot_is_cleared() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        super::super::refresh::tests::attach(editor);
        editor.change(|preset| preset.rename("Keep this edit"));
        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let source: Source = |_, _, _, _, publish| {
            publish(FollowUpdate::Connected, Instant::now());
            publish(FollowUpdate::Disconnected, Instant::now());
            publish(FollowUpdate::Connected, Instant::now());
            Ok(ListenEnd::Stopped)
        };

        editor.follow.start(source, &ctx).unwrap();
        let started = Instant::now();
        while !editor.follow.worker.as_ref().unwrap().thread.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
        poll_follow(editor, &ctx);
        assert!(!editor.follow.busy());
        assert!(editor.follow.disconnected());
        let key = editor.follow.attachment.as_ref().unwrap().key.clone();
        assert!(matches!(
            editor.take_device_event(),
            Some(DeviceEvent::GuitarDisconnected(notice_key)) if notice_key == key
        ));
        assert!(editor.take_device_event().is_none());
        assert!(editor.follow.start(source, &ctx).is_err());
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), before);

        editor.observe_device(None);
        assert!(editor.follow.disconnected());
        super::super::refresh::tests::attach(editor);
        assert!(!editor.follow.disconnected());
        assert!(!editor.follow.busy());
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn release_error_survives_invalidation_of_the_disconnected_attachment() {
        use crate::editor::detection::DeviceSupport;

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        let mut tab = refresh_tests::verified_tab();
        editor.observe_device(Some(&tab));
        editor
            .follow
            .start(
                |_, stop, _, ready, publish| {
                    ready();
                    publish(FollowUpdate::Disconnected, Instant::now());
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::yield_now();
                    }
                    Err("USB release (interface 1): driver return failed".into())
                },
                &ctx,
            )
            .unwrap();
        poll_until(editor, &ctx, Follow::disconnected);
        assert!(editor.follow.busy());
        let verified = std::mem::replace(&mut tab.device.support, DeviceSupport::UsbOnly);
        editor.observe_device(Some(&tab));
        poll_until(editor, &ctx, |follow| !follow.busy());
        assert_eq!(
            editor.follow.error.as_deref(),
            Some("USB release (interface 1): driver return failed"),
        );
        tab.device.support = verified;
        editor.observe_device(Some(&tab));
        assert!(editor.follow.error.is_none());
    }

    #[test]
    fn yielded_listener_keeps_the_display_and_expires_slot_evidence() {
        static YIELD: AtomicBool = AtomicBool::new(false);

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        refresh_tests::attach(editor);
        let follow = &mut editor.follow;
        follow
            .start(
                |_, stop, _, ready, publish| {
                    ready();
                    let now = Instant::now();
                    publish(FollowUpdate::Volume(64), now);
                    publish(FollowUpdate::ProgramChange(Preset::new(0, 0).unwrap()), now);
                    while !stop.load(Ordering::Relaxed)
                        && !YIELD.load(Ordering::Relaxed)
                        && now.elapsed() < Duration::from_secs(2)
                    {
                        std::thread::yield_now();
                    }
                    Ok(ListenEnd::Yielded)
                },
                &ctx,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let attachment = follow.attachment.clone().unwrap();
        let observation = loop {
            if let Some(observation) = follow.observed_slot(&attachment) {
                break observation;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };

        YIELD.store(true, Ordering::Relaxed);
        while !follow.worker.as_ref().unwrap().thread.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        follow.listen(&ctx);
        assert_eq!(follow.snapshot.volume, Some(64));
        assert!(follow.snapshot.program.is_some());
        assert_eq!(follow.consumed_program, follow.snapshot.program);
        assert!(!observation.still_valid());
        assert!(follow.observed_slot(&attachment).is_none());
    }

    #[test]
    fn latest_program_uses_workbench_settling_for_the_destination_slot() {
        let ctx = egui::Context::default();
        let mut follow = Follow::default();
        let mut document = EditorDocument::from_bundle(
            BundleFile::new_default_bundle().unwrap(),
            "test.vwb".into(),
        )
        .unwrap();
        let now = Instant::now();
        let preset = Preset::new(1, 4).unwrap();
        follow
            .snapshot
            .receive(FollowUpdate::ProgramChange(preset), now);
        follow
            .snapshot
            .receive(FollowUpdate::TonePosition(12), now + CONTROLLER_SETTLE);
        assert_eq!(follow.snapshot.tone, None);
        follow.snapshot.receive(
            FollowUpdate::TonePosition(42),
            now + CONTROLLER_SETTLE + Duration::from_millis(1),
        );
        assert_eq!(follow.snapshot.tone, Some(42));

        assert_eq!(
            follow.take_settled_slot(&document, now + Duration::from_millis(299), &ctx),
            None
        );
        assert_eq!(
            follow.take_settled_slot(&document, now + SLOT_SETTLE, &ctx),
            Some((1, 4))
        );
        assert_eq!(
            follow.take_settled_slot(&document, now + SLOT_SETTLE, &ctx),
            None
        );

        document.select(2, 2).unwrap();
        document
            .apply(|preset| preset.rename("Edited target"))
            .unwrap();
        document.select(0, 0).unwrap();
        let folder = tempfile::tempdir().unwrap();
        document
            .save_copy(&folder.path().join("edited.vwb"))
            .unwrap();
        assert!(!document.slot_is_dirty(2, 2));
        assert!(document.slot_is_unsent(2, 2));
        let next = now + Duration::from_secs(2);
        follow.snapshot.receive(
            FollowUpdate::ProgramChange(Preset::new(2, 2).unwrap()),
            next,
        );
        assert_eq!(
            follow.take_settled_slot(&document, next + Duration::from_millis(999), &ctx),
            None
        );
        assert_eq!(
            follow.take_settled_slot(&document, next + EDITED_SLOT_SETTLE, &ctx),
            Some((2, 2))
        );
    }

    #[test]
    fn tuning_override_renders_both_states_without_changing_the_saved_tuning() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        let document = editor.document.as_ref().unwrap();
        let before = document.as_bytes().to_vec();
        let undo = document.can_undo();

        fn count(shape: &egui::Shape, label: &str) -> usize {
            match shape {
                egui::Shape::Text(text) => usize::from(text.galley.text() == label),
                egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, label)).sum(),
                _ => 0,
            }
        }

        for (enabled, expected) in [
            (true, "Guitar: alternate tuning on"),
            (false, "Guitar: alternate tuning off"),
        ] {
            editor
                .follow
                .snapshot
                .receive(FollowUpdate::TuningOverride(enabled), Instant::now());
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| editor.test_fretboard(ui, true));
            });
            assert_eq!(
                output
                    .shapes
                    .iter()
                    .map(|item| count(&item.shape, expected))
                    .sum::<usize>(),
                1,
            );
            let document = editor.document.as_ref().unwrap();
            assert_eq!(document.as_bytes(), before);
            assert_eq!(document.can_undo(), undo);
        }

        editor.follow.stop();
        assert_eq!(editor.follow.tuning_label(), None);
    }
}

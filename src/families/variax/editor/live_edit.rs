//! Workbench editor-side scheduling. The device adapter is supplied by #245.
use super::{
    FamilyTab, PresetFile, TUNING, VariaxEditor, egui, follow::ObservedSlot,
    live_attachment::Attachment,
};
use line6_devices::family::variax::{Preset, procedures::Image};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use variax_workbench::hardware_gates::{Gate, run_if_unlocked};

const LIVE_EDIT_SEND_LOCKED: bool = true;
const MANUAL_EDIT_BUFFER_SEND_LOCKED: bool = true;
const DEBOUNCE: Duration = Duration::from_millis(120);

/// The adapter holds the rig lock, re-verifies the
/// attachment, and admits only reference-verified programs. A send result
/// means the working buffer was verified. All operations honor cancellation
/// and release the session before returning. No request addresses flash.
pub(super) trait EditBufferDevice: Send + Sync {
    fn send(
        &self,
        attachment: &Attachment,
        model: &PresetFile,
        previous: Option<&Previous>,
        cancel: &AtomicBool,
    ) -> Result<Image, String>;

    fn recall(
        &self,
        attachment: &Attachment,
        request: &Recall,
        cancel: &AtomicBool,
    ) -> Result<Option<Image>, String>;

    fn get(
        &self,
        attachment: &Attachment,
        cancel: &AtomicBool,
    ) -> Result<(PresetFile, Image), String>;
}

pub(super) struct Previous {
    pub(super) image: Box<Image>,
    pub(super) tuning_transition: bool,
}

pub(super) struct Recall {
    pub(super) current: ObservedSlot,
    pub(super) target: Preset,
    pub(super) starting_image: Box<Image>,
    pub(super) target_image: Box<Image>,
}

enum Request {
    Send {
        model: PresetFile,
        previous: Option<Previous>,
    },
    Recall(Recall),
    Get,
}

enum Reply {
    Verified(Box<Image>),
    SelectionLocal,
    Current(PresetFile, Box<Image>),
}

#[derive(Clone)]
struct Model {
    generation: u64,
    position: Option<(usize, usize)>,
    preset: PresetFile,
}

impl Model {
    fn same_location(&self, other: &Self) -> bool {
        self.generation == other.generation && self.position == other.position
    }

    fn same(&self, other: &Self) -> bool {
        self.same_location(other) && self.preset.as_bytes() == other.preset.as_bytes()
    }

    fn tuning_enabled(&self) -> Option<bool> {
        self.preset.model(TUNING).map(|model| model.enabled)
    }
}

#[derive(Clone, Copy)]
enum Action {
    Automatic,
    Select,
    Recall,
    Send,
    Get,
}

struct Work {
    action: Action,
    model: Option<Model>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<Result<Reply, String>>,
}

#[derive(Default)]
pub(super) struct LiveEdit {
    device: Option<Arc<dyn EditBufferDevice>>,
    attachment: Option<Attachment>,
    enabled: bool,
    generation: u64,
    seen: Option<Model>,
    verified: Option<(Model, Box<Image>)>,
    stored: HashMap<u8, (PresetFile, Box<Image>)>,
    queued: Option<(Model, Instant)>,
    worker: Option<Work>,
    #[cfg(test)]
    fake_unlocked: bool,
}

impl LiveEdit {
    pub(super) fn new(_preferences: Option<&std::path::Path>) -> Self {
        #[cfg(all(target_os = "linux", not(test)))]
        {
            let mut state = Self::default();
            state.device = Some(Arc::new(super::procedure_device::EditorDevice::new(
                _preferences,
            )));
            state
        }
        #[cfg(any(not(target_os = "linux"), test))]
        Self::default()
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn selecting(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|work| matches!(work.action, Action::Select | Action::Recall))
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn observe(&mut self, attachment: Option<Attachment>) {
        if self.attachment != attachment {
            self.stop();
            self.forget_stored_images();
            self.attachment = attachment;
        }
    }

    pub(super) fn new_document(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.forget_stored_images();
        if let Some(work) = &self.worker {
            work.cancel.store(true, Ordering::Relaxed);
        }
        self.seen = None;
        self.queued = None;
        self.verified = None;
    }

    pub(super) fn document_generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn forget_stored_images(&mut self) {
        self.stored.clear();
    }

    pub(super) fn remember_stored_images(
        &mut self,
        images: Vec<(Preset, Box<Image>)>,
        models: &[(usize, usize, PresetFile)],
    ) {
        for (position, image) in images {
            if let Some((_, _, model)) = models.iter().find(|(bank, slot, _)| {
                (*bank, *slot) == (usize::from(position.bank()), usize::from(position.slot()))
            }) {
                self.stored
                    .insert(position.program_number(), (model.clone(), image));
            }
        }
    }

    pub(super) fn stop(&mut self) {
        self.enabled = false;
        self.seen = None;
        self.queued = None;
        self.verified = None;
        if let Some(work) = &self.worker {
            work.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn permits(&self, action: Action) -> bool {
        #[cfg(test)]
        if self.fake_unlocked {
            return true;
        }
        match action {
            Action::Automatic | Action::Select | Action::Recall => !LIVE_EDIT_SEND_LOCKED,
            Action::Send => !MANUAL_EDIT_BUFFER_SEND_LOCKED,
            Action::Get => true,
        }
    }

    fn ready(&self, action: Action) -> bool {
        self.permits(action) && self.device.is_some() && self.attachment.is_some()
    }

    fn stage(&mut self, model: Model, now: Instant) {
        if self.seen.as_ref().is_some_and(|seen| seen.same(&model)) {
            return;
        }
        let deadline = match &self.seen {
            Some(seen) if seen.same_location(&model) => now + DEBOUNCE,
            _ => now,
        };
        self.seen = Some(model.clone());
        self.queued = Some((model, deadline));
    }

    fn start(
        &mut self,
        action: Action,
        model: Option<Model>,
        recall: Option<Recall>,
    ) -> Result<(), String> {
        if self.busy() || !self.ready(action) {
            return Err("The Variax edit buffer is unavailable.".into());
        }
        let device = Arc::clone(self.device.as_ref().expect("verified adapter"));
        let attachment = self.attachment.clone().expect("verified attachment");
        let request = match action {
            Action::Get => Request::Get,
            Action::Recall => Request::Recall(recall.ok_or("Saved model evidence expired.")?),
            _ => {
                let target = model.as_ref().ok_or("Select a Variax model first.")?;
                let previous = self.verified.as_ref().and_then(|(before, image)| {
                    (matches!(action, Action::Automatic) && before.same_location(target)).then(
                        || Previous {
                            image: image.clone(),
                            tuning_transition: before.tuning_enabled() != target.tuning_enabled(),
                        },
                    )
                });
                Request::Send {
                    model: target.preset.clone(),
                    previous,
                }
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let gate = match action {
            Action::Get => Gate::CurrentSettingsRead,
            _ => Gate::EditBufferWrite,
        };
        let thread = std::thread::Builder::new()
            .name("variax-edit-buffer".into())
            .spawn(move || {
                run_if_unlocked(gate, || {
                    if worker_cancel.load(Ordering::Relaxed) {
                        return Err("Edit-buffer operation cancelled.".into());
                    }
                    match request {
                        Request::Send { model, previous } => device
                            .send(&attachment, &model, previous.as_ref(), &worker_cancel)
                            .map(|image| Reply::Verified(Box::new(image))),
                        Request::Recall(request) => {
                            match device.recall(&attachment, &request, &worker_cancel)? {
                                None => Ok(Reply::SelectionLocal),
                                Some(image) if request.current.still_valid() => {
                                    Ok(Reply::Verified(Box::new(image)))
                                }
                                Some(_) => Err("The current slot changed during recall. The active guitar model could not be verified.".into()),
                            }
                        }
                        Request::Get => device
                            .get(&attachment, &worker_cancel)
                            .map(|(preset, image)| Reply::Current(preset, Box::new(image))),
                    }
                })
            })
            .map_err(|error| format!("Could not start the Variax edit-buffer worker: {error}"))?;
        self.worker = Some(Work {
            action,
            model,
            generation: self.generation,
            cancel,
            thread,
        });
        Ok(())
    }
}

impl Drop for LiveEdit {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.thread.join();
        }
    }
}

impl VariaxEditor {
    fn live_model(&self) -> Option<Model> {
        let document = self.document.as_ref()?;
        let preset = document
            .selected_preset()?
            .is_legacy_electric()
            .then(|| document.preset().clone())?;
        Some(Model {
            generation: self.live_edit.generation,
            position: document.selection(),
            preset,
        })
    }

    pub(super) fn live_selection_followed(&mut self) {
        self.live_edit.seen = self.live_model();
        self.live_edit.queued = None;
        self.live_edit.verified = None;
    }

    pub(super) fn live_select_explicit(&mut self) {
        self.live_selection_followed();
        if !self.live_edit.enabled()
            || !self.edit_buffer_available()
            || self.live_edit.busy()
            || !self.live_edit.ready(Action::Select)
            || !self.has_edit_buffer_model()
        {
            return;
        }
        let Some(model) = self.live_model() else {
            return;
        };
        let document = self.document.as_ref().expect("selected live model");
        let recall = match model.position {
            Some((bank, slot))
                if self.has_current_guitar_library() && !document.slot_is_unsent(bank, slot) =>
            {
                let recall = self.recall_request(bank, slot, &model.preset);
                if recall.is_none() {
                    self.status =
                        "Selected locally. Saved recall needs verified guitar positions.".into();
                    return;
                }
                recall
            }
            _ => None,
        };
        let action = if recall.is_some() {
            Action::Recall
        } else {
            Action::Select
        };
        self.start_edit_buffer_with(action, Some(model), recall);
    }

    fn recall_request(&self, bank: usize, slot: usize, model: &PresetFile) -> Option<Recall> {
        let target = Preset::new(u8::try_from(bank).ok()?, u8::try_from(slot).ok()?)?;
        let (saved_model, target_image) = self.live_edit.stored.get(&target.program_number())?;
        if saved_model.as_bytes() != model.as_bytes() {
            return None;
        }
        let current = self
            .follow
            .observed_slot(self.live_edit.attachment.as_ref()?)?;
        let (_, starting_image) = self
            .live_edit
            .stored
            .get(&current.preset.program_number())?;
        Some(Recall {
            current,
            target,
            starting_image: starting_image.clone(),
            target_image: target_image.clone(),
        })
    }

    pub(super) fn live_paste_completed(&mut self) {
        // Copy/Paste changes the library locally. The next explicit edit or
        // model selection can send again from an unknown buffer baseline.
        let enabled = self.live_edit.enabled();
        self.live_edit.stop();
        self.live_edit.enabled = enabled;
        self.live_edit.seen = self.live_model();
    }

    fn edit_buffer_available(&self) -> bool {
        !self.device_sync.stopped_after_store()
            && !self.is_reading()
            && !self.follow.disconnected()
            && !self.blocking_prompt()
    }

    fn has_edit_buffer_model(&self) -> bool {
        !self.has_historical_guitar_library()
            && self
                .document
                .as_ref()
                .and_then(|doc| doc.selected_preset())
                .is_some_and(PresetFile::is_legacy_electric)
    }

    pub(super) fn live_edit_toggle(&mut self, ui: &mut egui::Ui) {
        let mut enabled = self.live_edit.enabled();
        let can_enable = self.edit_buffer_available()
            && self.has_edit_buffer_model()
            && self.live_edit.ready(Action::Automatic);
        if ui
            .add_enabled(
                enabled || can_enable,
                egui::Checkbox::new(&mut enabled, "Live editing"),
            )
            .on_disabled_hover_text("Live editing awaits its first owner test.")
            .changed()
        {
            if enabled {
                self.live_edit.enabled = true;
            } else {
                self.live_edit.stop();
            }
        }
    }

    pub(super) fn edit_buffer_actions(&mut self, ui: &mut egui::Ui) {
        let idle = self.edit_buffer_available() && !self.live_edit.busy();
        if ui
            .add_enabled(
                idle && self.has_edit_buffer_model() && self.live_edit.ready(Action::Send),
                egui::Button::new("Send Edit Buffer"),
            )
            .clicked()
        {
            self.start_edit_buffer(Action::Send, self.live_model());
            ui.close();
        }
        if ui
            .add_enabled(
                idle && self.live_edit.ready(Action::Get),
                egui::Button::new("Read current model from guitar…"),
            )
            .on_hover_text("Read the guitar’s working buffer (Get Edit Buffer in Workbench 1.75). Opens an editable model. Use Save As to write a file.")
            .clicked()
        {
            if self.dirty() {
                self.pending = Some(super::GuitarPending::Open(
                    super::OpenDestination::EditBuffer,
                ));
            } else {
                self.get_edit_buffer();
            }
            ui.close();
        }
    }

    pub(super) fn get_edit_buffer(&mut self) {
        if !self.device_sync.stopped_after_store()
            && !self.is_reading()
            && !self.follow.disconnected()
        {
            self.start_edit_buffer(Action::Get, self.live_model());
        }
    }

    fn start_edit_buffer(&mut self, action: Action, model: Option<Model>) {
        self.start_edit_buffer_with(action, model, None);
    }

    fn start_edit_buffer_with(
        &mut self,
        action: Action,
        model: Option<Model>,
        recall: Option<Recall>,
    ) {
        if !matches!(action, Action::Get) && self.has_historical_guitar_library() {
            return;
        }
        match self.live_edit.start(action, model, recall) {
            Ok(()) => {
                self.error = None;
                self.status = match action {
                    Action::Get => "Reading guitar working buffer…",
                    Action::Recall => "Recalling saved guitar model…",
                    _ => "Updating guitar working buffer…",
                }
                .into();
            }
            Err(error) => {
                self.live_edit.stop();
                self.error = Some(error);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn hold_fake_edit(&mut self) -> std::sync::mpsc::Sender<()> {
        let (finish, wait) = std::sync::mpsc::channel();
        self.live_edit.worker = Some(Work {
            action: Action::Send,
            model: self.live_model(),
            generation: self.live_edit.generation,
            cancel: Arc::new(AtomicBool::new(false)),
            thread: std::thread::spawn(move || {
                let _ = wait.recv();
                Ok(Reply::Verified(Box::new(
                    [0; line6_devices::family::variax::procedures::PROGRAM_BYTES],
                )))
            }),
        });
        finish
    }

    pub(super) fn poll_edit_buffer(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.advance_edit_buffer(now);
        if self.live_edit.enabled() || self.live_edit.busy() {
            let delay = self.live_edit.queued.as_ref().map_or(
                Duration::from_millis(35),
                |(_, deadline)| {
                    deadline
                        .saturating_duration_since(now)
                        .clamp(Duration::from_millis(10), Duration::from_millis(35))
                },
            );
            ctx.request_repaint_after(delay);
        }
    }

    fn advance_edit_buffer(&mut self, now: Instant) {
        if self.follow.disconnected() {
            self.live_edit.stop();
            self.live_edit.forget_stored_images();
        }
        if self
            .live_edit
            .worker
            .as_ref()
            .is_some_and(|work| work.thread.is_finished())
        {
            self.finish_edit_buffer();
        }
        if !self.live_edit.enabled()
            || self.has_historical_guitar_library()
            || self.is_reading()
            || self.opening_file
            || self.blocking_prompt()
        {
            return;
        }
        if self
            .document
            .as_ref()
            .is_some_and(|document| self.follow.pending_selection(document))
        {
            self.live_selection_followed();
            return;
        }
        if let Some(model) = self.live_model() {
            self.live_edit.stage(model, now);
        } else {
            self.live_edit.queued = None;
            self.live_edit.seen = None;
        }
        if !self.live_edit.busy()
            && self
                .live_edit
                .queued
                .as_ref()
                .is_some_and(|(_, deadline)| *deadline <= now)
        {
            let (model, _) = self.live_edit.queued.take().expect("ready update");
            if !self
                .live_edit
                .verified
                .as_ref()
                .is_some_and(|(verified, _)| verified.same(&model))
            {
                self.start_edit_buffer(Action::Automatic, Some(model));
            }
        }
    }

    fn finish_edit_buffer(&mut self) {
        let work = self
            .live_edit
            .worker
            .take()
            .expect("finished edit-buffer worker");
        let result = work
            .thread
            .join()
            .unwrap_or_else(|_| Err("The Variax edit-buffer worker stopped unexpectedly.".into()));
        if work.cancel.load(Ordering::Relaxed) || work.generation != self.live_edit.generation {
            self.status = match work.action {
                Action::Get => "Guitar working-buffer read stopped.",
                _ => "Guitar operation stopped. The active model was not verified.",
            }
            .into();
            self.error = result.err();
            return;
        }
        match (work.action, result) {
            (Action::Recall, Ok(Reply::SelectionLocal)) => {
                self.status =
                    "Selected locally. The guitar slot could not be verified through the handoff."
                        .into();
            }
            (
                Action::Automatic | Action::Send | Action::Select | Action::Recall,
                Ok(Reply::Verified(image)),
            ) => {
                self.live_edit.verified = work.model.map(|model| (model, image));
                self.status = if matches!(work.action, Action::Recall) {
                    "Saved guitar model recalled and verified."
                } else {
                    "Guitar working buffer verified."
                }
                .into();
            }
            (Action::Get, Ok(Reply::Current(preset, image))) if preset.is_legacy_electric() => {
                let current = self.live_model();
                let unchanged = match (&work.model, current.as_ref()) {
                    (Some(before), Some(after)) => before.same(after),
                    (None, None) => true,
                    _ => false,
                };
                if self.live_edit.generation != work.generation || !unchanged {
                    self.error =
                        Some("The open model changed during the read. The editor kept it.".into());
                    self.live_edit.stop();
                    return;
                }
                self.adopt_opened_document(
                    super::EditorDocument::from_preset(preset, "Guitar working buffer".into()),
                    None,
                );
                let model = self.live_model();
                self.live_edit.seen = model.clone();
                self.live_edit.verified = model.map(|model| (model, image));
                self.status = "Read the guitar working buffer.".into();
            }
            (_, result) => {
                self.live_edit.stop();
                self.error = Some(
                    result
                        .err()
                        .unwrap_or_else(|| "Unexpected Variax edit-buffer result.".into()),
                );
                if matches!(work.action, Action::Select | Action::Recall) {
                    self.status =
                        "Selected locally. The active guitar model was not verified.".into();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tests::app;
    use std::sync::{Mutex, mpsc};

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Full(String),
        Transition(String),
        Difference { before: String, after: String },
        Recall(Preset, Preset),
        Get,
    }

    struct Fake {
        attachment: Attachment,
        calls: Mutex<Vec<Call>>,
        result: PresetFile,
        cancelled: Option<mpsc::Sender<()>>,
        recall_error: Mutex<Option<&'static str>>,
        recall_gap: AtomicBool,
    }

    impl EditBufferDevice for Fake {
        fn send(
            &self,
            attachment: &Attachment,
            model: &PresetFile,
            previous: Option<&Previous>,
            cancel: &AtomicBool,
        ) -> Result<Image, String> {
            assert!(attachment == &self.attachment);
            let call = match previous {
                Some(previous) if previous.tuning_transition => {
                    Call::Transition(model.name().into())
                }
                Some(previous) => {
                    let name = &previous.image[1..=usize::from(previous.image[0])];
                    Call::Difference {
                        before: String::from_utf8_lossy(name).into(),
                        after: model.name().into(),
                    }
                }
                None => Call::Full(model.name().into()),
            };
            self.calls.lock().unwrap().push(call);
            if let Some(ack) = &self.cancelled {
                while !cancel.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                ack.send(()).unwrap();
            }
            Ok(fake_image(model.name()))
        }

        fn recall(
            &self,
            attachment: &Attachment,
            request: &Recall,
            _: &AtomicBool,
        ) -> Result<Option<Image>, String> {
            assert!(attachment == &self.attachment);
            if self.recall_gap.load(Ordering::Relaxed) {
                request.current.interrupt();
            }
            if !request.current.still_valid() {
                return Ok(None);
            }
            self.calls
                .lock()
                .unwrap()
                .push(Call::Recall(request.current.preset, request.target));
            match *self.recall_error.lock().unwrap() {
                Some(error) => Err(error.into()),
                None => Ok(Some(*request.target_image)),
            }
        }

        fn get(
            &self,
            attachment: &Attachment,
            _: &AtomicBool,
        ) -> Result<(PresetFile, Image), String> {
            assert!(attachment == &self.attachment);
            self.calls.lock().unwrap().push(Call::Get);
            Ok((self.result.clone(), fake_image(self.result.name())))
        }
    }

    fn fake_image(name: &str) -> Image {
        let mut image = [0; line6_devices::family::variax::procedures::PROGRAM_BYTES];
        image[0] = name.len() as u8;
        image[1..=name.len()].copy_from_slice(name.as_bytes());
        image
    }

    fn connect(editor: &mut VariaxEditor, cancelled: Option<mpsc::Sender<()>>) -> Arc<Fake> {
        let tab = super::super::refresh::tests::verified_tab();
        let mut preset = editor.document.as_ref().unwrap().preset().clone();
        preset.rename("Working buffer").unwrap();
        let device = Arc::new(Fake {
            attachment: Attachment::verified(&tab).unwrap(),
            calls: Mutex::default(),
            result: preset,
            cancelled,
            recall_error: Mutex::default(),
            recall_gap: AtomicBool::new(false),
        });
        editor.observe_device(Some(&tab));
        editor.live_edit.device = Some(device.clone());
        device
    }

    fn complete(editor: &mut VariaxEditor, now: Instant) {
        let timeout = Instant::now() + Duration::from_secs(2);
        while editor
            .live_edit
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.thread.is_finished())
        {
            assert!(Instant::now() < timeout);
            std::thread::yield_now();
        }
        editor.advance_edit_buffer(now);
    }

    #[test]
    fn physical_selection_clears_a_queued_live_edit_without_uploading_either_slot() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.open_example();
        let device = connect(editor, None);
        super::super::refresh::tests::download(editor, &ctx);
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        editor.live_selection_followed();
        editor.change(|preset| preset.rename("Keep edited source"));
        let now = Instant::now();
        editor.advance_edit_buffer(now);
        assert!(editor.live_edit.queued.is_some());

        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
        editor.follow.test_program(Preset::new(4, 2).unwrap(), now);
        editor.advance_follow(&ctx);
        editor.advance_edit_buffer(now + DEBOUNCE);
        assert!(editor.live_edit.queued.is_none());

        editor
            .follow
            .test_program(Preset::new(4, 2).unwrap(), now - Duration::from_secs(2));
        editor.advance_follow(&ctx);
        editor.advance_edit_buffer(now + Duration::from_secs(1));
        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((4, 2)));
        assert_eq!(document.as_bytes(), before);
        assert!(document.is_dirty() && document.can_undo());
        assert!(device.calls.lock().unwrap().is_empty());
        assert!(editor.live_edit.enabled());
    }

    #[test]
    fn live_pointer_bank_and_keyboard_slot_use_full_upload_for_local_drafts() {
        use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
        let (ctx, mut app) = app();
        ctx.style_mut(|style| style.animation_time = 0.);
        app.variax_mut().open_example();
        let device = connect(app.variax_mut(), None);
        let frame = |app: &mut crate::editor::Workbench, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
                },
            );
        };
        let click = |app: &mut crate::editor::Workbench, pos| {
            for pressed in [true, false] {
                frame(
                    app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            frame(app, vec![]);
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        // The closed switch permits normal local pointer selection.
        let slot = app.variax().bank_slots[&0].center();
        click(&mut app, slot);
        assert_eq!(
            app.variax().document.as_ref().unwrap().selection(),
            Some((0, 0))
        );
        assert!(device.calls.lock().unwrap().is_empty());

        let editor = app.variax_mut();
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        editor.change(|preset| preset.rename("Keep local draft"));
        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let combo = app.variax().bank_dropdown.unwrap().center();
        click(&mut app, combo);
        let option = app.variax().bank_options[&1].center();
        click(&mut app, option);
        let editor = app.variax_mut();
        assert_eq!(editor.document.as_ref().unwrap().selection(), Some((1, 0)));
        let bank_model = editor.document.as_ref().unwrap().preset().name().to_owned();
        complete(editor, Instant::now());
        frame(&mut app, vec![]);

        let target = app.variax().bank_slots[&2];
        let tap = |app: &mut crate::editor::Workbench, key| {
            for pressed in [true, false] {
                frame(
                    app,
                    vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                );
            }
        };
        for _ in 0..80 {
            if ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id))
                .is_some_and(|response| response.rect == target)
            {
                break;
            }
            tap(&mut app, Key::Tab);
        }
        tap(&mut app, Key::Enter);
        let editor = app.variax_mut();
        complete(editor, Instant::now());
        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((1, 2)));
        assert_eq!(document.as_bytes(), before);
        assert!(document.slot_is_dirty(0, 0));
        assert_eq!(
            *device.calls.lock().unwrap(),
            [
                Call::Full(bank_model),
                Call::Full(document.preset().name().into())
            ]
        );
    }

    #[test]
    fn only_current_attachment_and_exact_synchronized_slots_route_to_recall() {
        use super::super::{LibraryOrigin, refresh::tests};
        use std::time::SystemTime;
        use variax_workbench::bundle::BundleFile;

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.open_example();
        let device = connect(editor, None);
        let key = device.attachment.key.clone();
        let mut rig = tests::verified_rig();
        let first = tests::verified_tab_for(&mut rig);
        let profiles = crate::editor::registry::DEVICE_PROFILES;
        rig.report_guitar_disconnect(&first.device.key, profiles)
            .unwrap();
        assert!(rig.queue_guitar_refresh(&first.device.key.attachment, false));
        let other_key = tests::verified_tab_for(&mut rig).device.key;
        assert!(key != other_key);
        editor
            .adopt_startup_library(
                BundleFile::new_default_bundle().unwrap(),
                LibraryOrigin::AttachedDownload(SystemTime::UNIX_EPOCH, key.clone()),
            )
            .unwrap();
        let document = editor.document.as_ref().unwrap();
        let models: Vec<_> = (0..=1)
            .map(|slot| {
                (
                    0,
                    slot,
                    document.banks().unwrap()[0].presets()[slot].clone(),
                )
            })
            .collect();
        // Duplicate images cannot stand in for a missing independent selector event.
        let identical = Box::new(fake_image("Same sound"));
        let images = (0..=1)
            .map(|slot| (Preset::new(0, slot).unwrap(), identical.clone()))
            .collect();
        editor.live_edit.remember_stored_images(images, &models);
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        editor.document.as_mut().unwrap().select(0, 1).unwrap();
        editor.live_select_explicit();
        assert!(device.calls.lock().unwrap().is_empty());

        editor.follow.listen_for_test_slot(&ctx);
        editor.library_origin = Some(LibraryOrigin::AttachedDownload(
            SystemTime::UNIX_EPOCH,
            other_key,
        ));
        editor.live_select_explicit();
        assert!(device.calls.lock().unwrap().is_empty());

        editor.library_origin = Some(LibraryOrigin::AttachedDownload(SystemTime::UNIX_EPOCH, key));
        *device.recall_error.lock().unwrap() =
            Some("Injected mismatch and checked release failure");
        editor.live_select_explicit();
        complete(editor, Instant::now());
        let expected = Call::Recall(Preset::new(0, 0).unwrap(), Preset::new(0, 1).unwrap());
        assert_eq!(*device.calls.lock().unwrap(), [expected]);
        assert_eq!(
            editor.error.as_deref(),
            Some("Injected mismatch and checked release failure")
        );
        assert!(
            editor
                .status
                .contains("active guitar model was not verified")
        );
        assert!(!editor.live_edit.enabled());
        assert_eq!(editor.document.as_ref().unwrap().selection(), Some((0, 1)));

        editor.change(|preset| preset.rename("Edited slot"));
        editor.live_edit.enabled = true;
        editor.live_select_explicit();
        complete(editor, Instant::now());
        assert_eq!(
            device.calls.lock().unwrap().last(),
            Some(&Call::Full("Edited slot".into()))
        );
        assert!(editor.dirty());
        editor.undo();
        device.recall_gap.store(true, Ordering::Relaxed);
        let before = device.calls.lock().unwrap().len();
        editor.live_select_explicit();
        complete(editor, Instant::now());
        assert_eq!(device.calls.lock().unwrap().len(), before);
        assert!(
            editor
                .status
                .contains("could not be verified through the handoff")
        );
        assert!(editor.live_edit.enabled());
        assert!(editor.error.is_none());
        editor.observe_device(None);
        assert!(editor.live_edit.stored.is_empty());
        assert!(editor.live_edit.verified.is_none());
    }

    #[test]
    fn normal_download_and_reopened_startup_library_supply_no_exact_recall_images() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.open_example();
        let device = connect(editor, None);
        super::super::refresh::tests::download(editor, &ctx);
        assert!(editor.has_current_guitar_library());
        assert!(editor.live_edit.stored.is_empty());
        editor.follow.listen_for_test_slot(&ctx);
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        editor.document.as_mut().unwrap().select(3, 3).unwrap();
        editor.live_select_explicit();
        assert!(editor.status.contains("needs verified guitar positions"));
        assert!(!editor.live_edit.busy());

        let bundle = variax_workbench::bundle::BundleFile::new_default_bundle().unwrap();
        let origin = super::super::LibraryOrigin::Download(std::time::SystemTime::UNIX_EPOCH);
        editor.adopt_startup_library(bundle, origin).unwrap();
        assert!(editor.has_historical_guitar_library());
        editor.live_select_explicit();
        assert!(!editor.live_edit.busy());
        assert!(device.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn live_edits_debounce_differences_and_route_model_loads_and_tuning_toggles() {
        let (_, mut app) = app();
        let editor = app.variax_mut();
        editor.open_example();
        let device = connect(editor, None);
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        let now = Instant::now();
        let first = editor.document.as_ref().unwrap().preset().name().to_owned();
        editor.advance_edit_buffer(now);
        complete(editor, now);
        assert_eq!(*device.calls.lock().unwrap(), [Call::Full(first.clone())]);

        editor.change(|preset| preset.rename("First edit"));
        let first_edit = now + Duration::from_millis(10);
        editor.advance_edit_buffer(first_edit);
        editor.change(|preset| preset.rename("Debounced edit"));
        let last_edit = now + Duration::from_millis(70);
        editor.advance_edit_buffer(last_edit);
        editor.advance_edit_buffer(first_edit + DEBOUNCE);
        assert!(!editor.live_edit.busy());
        assert_eq!(device.calls.lock().unwrap().len(), 1);
        editor.advance_edit_buffer(last_edit + DEBOUNCE);
        complete(editor, last_edit + DEBOUNCE);
        assert_eq!(
            device.calls.lock().unwrap()[1],
            Call::Difference {
                before: first,
                after: "Debounced edit".into()
            }
        );
        assert!(
            editor
                .document
                .as_ref()
                .unwrap()
                .selection()
                .is_some_and(|(bank, slot)| editor
                    .document
                    .as_ref()
                    .unwrap()
                    .slot_is_unsent(bank, slot))
        );

        let enabled = editor
            .document
            .as_ref()
            .unwrap()
            .preset()
            .model(TUNING)
            .unwrap()
            .enabled;
        editor.change(|preset| preset.set_enabled(TUNING, !enabled));
        let tuning = now + DEBOUNCE * 3;
        editor.advance_edit_buffer(tuning);
        editor.advance_edit_buffer(tuning + DEBOUNCE);
        complete(editor, tuning + DEBOUNCE);
        assert_eq!(
            device.calls.lock().unwrap()[2],
            Call::Transition("Debounced edit".into())
        );

        editor.document.as_mut().unwrap().select(0, 3).unwrap();
        let selected_name = editor.document.as_ref().unwrap().preset().name().to_owned();
        editor.advance_edit_buffer(tuning + DEBOUNCE * 2);
        complete(editor, tuning + DEBOUNCE * 2);
        assert_eq!(device.calls.lock().unwrap()[3], Call::Full(selected_name));

        let mut file = editor.document.as_ref().unwrap().preset().clone();
        file.rename("Opened file").unwrap();
        editor.adopt_opened_document(
            super::super::EditorDocument::from_preset(file, "opened.vwm".into()),
            Some(std::path::Path::new("opened.vwm")),
        );
        editor.advance_edit_buffer(tuning + DEBOUNCE * 3);
        complete(editor, tuning + DEBOUNCE * 3);
        assert_eq!(
            device.calls.lock().unwrap()[4],
            Call::Full("Opened file".into())
        );
    }

    #[test]
    fn closed_switches_refuse_sends_and_manual_get_keeps_the_library() {
        let (_, mut app) = app();
        let editor = app.variax_mut();
        editor.open_example();
        let device = connect(editor, None);
        let now = Instant::now();
        for action in [Action::Automatic, Action::Send] {
            assert!(
                editor
                    .live_edit
                    .start(action, editor.live_model(), None)
                    .is_err()
            );
        }
        assert!(device.calls.lock().unwrap().is_empty());

        editor.live_edit.fake_unlocked = true;
        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let initial = editor.document.as_ref().unwrap().preset().name().to_owned();
        editor.start_edit_buffer(Action::Send, editor.live_model());
        complete(editor, now);
        editor.get_edit_buffer();
        complete(editor, now);
        assert_eq!(
            *device.calls.lock().unwrap(),
            [Call::Full(initial), Call::Get]
        );
        assert_eq!(
            editor.document.as_ref().unwrap().preset().name(),
            "Working buffer"
        );
        assert_eq!(
            editor.parked_library.as_ref().unwrap().document.as_bytes(),
            before
        );
        assert!(!editor.dirty());
        assert!(!editor.live_edit.enabled);
    }

    #[test]
    fn historical_library_cannot_supply_the_working_buffer_send() {
        use super::super::LibraryOrigin;
        use std::time::SystemTime;

        let (_, mut app) = app();
        let editor = app.variax_mut();
        editor
            .adopt_startup_library(
                variax_workbench::bundle::BundleFile::new_default_bundle().unwrap(),
                LibraryOrigin::Download(SystemTime::UNIX_EPOCH),
            )
            .unwrap();
        let device = connect(editor, None);
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        let now = Instant::now();
        editor.start_edit_buffer(Action::Send, editor.live_model());
        editor.advance_edit_buffer(now);
        editor.advance_edit_buffer(now + DEBOUNCE);
        assert!(!editor.live_edit.busy());
        assert!(editor.live_edit.queued.is_none());
        assert!(device.calls.lock().unwrap().is_empty());

        editor.get_edit_buffer();
        complete(editor, now);
        assert_eq!(*device.calls.lock().unwrap(), [Call::Get]);
        editor.return_to_library();
        assert!(editor.has_historical_guitar_library());
    }

    #[test]
    fn disconnect_cancels_the_verified_attachment_worker_and_discards_queued_edits() {
        let (_, mut app) = app();
        let editor = app.variax_mut();
        let (tx, rx) = mpsc::channel();
        let device = connect(editor, Some(tx));
        editor.live_edit.fake_unlocked = true;
        editor.live_edit.enabled = true;
        let now = Instant::now();
        editor.advance_edit_buffer(now);
        let timeout = Instant::now() + Duration::from_secs(2);
        while device.calls.lock().unwrap().is_empty() {
            assert!(Instant::now() < timeout);
            std::thread::yield_now();
        }
        editor.change(|preset| preset.rename("Keep locally"));
        editor.advance_edit_buffer(now + DEBOUNCE);
        editor.observe_device(None);
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        complete(editor, now + DEBOUNCE * 2);
        assert_eq!(device.calls.lock().unwrap().len(), 1);
        assert!(editor.live_edit.queued.is_none() && editor.live_edit.verified.is_none());
        assert!(!editor.live_edit.enabled());
        assert!(editor.dirty());
    }
}

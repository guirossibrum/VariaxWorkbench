//! Workbench editor-side scheduling. The device adapter is supplied by #245.
use super::{FamilyTab, PresetFile, TUNING, VariaxEditor, egui, live_attachment::Attachment};
use line6_devices::family::variax::procedures::Image;
use std::{
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

enum Request {
    Send {
        model: PresetFile,
        previous: Option<Previous>,
    },
    Get,
}

enum Reply {
    Verified(Box<Image>),
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

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn observe(&mut self, attachment: Option<Attachment>) {
        if self.attachment != attachment {
            self.stop();
            self.attachment = attachment;
        }
    }

    pub(super) fn new_document(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.seen = None;
        self.queued = None;
        self.verified = None;
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
            Action::Automatic => !LIVE_EDIT_SEND_LOCKED,
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

    fn start(&mut self, action: Action, model: Option<Model>) -> Result<(), String> {
        if self.busy() || !self.ready(action) {
            return Err("The Variax edit buffer is unavailable.".into());
        }
        let device = Arc::clone(self.device.as_ref().expect("verified adapter"));
        let attachment = self.attachment.clone().expect("verified attachment");
        let request = match action {
            Action::Get => Request::Get,
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

    pub(super) fn edit_buffer_controls(&mut self, ui: &mut egui::Ui) {
        if self.connection_status.is_none() {
            return;
        }
        let free = !self.is_reading()
            && !self.follow.busy()
            && !self.follow.disconnected()
            && !self.blocking_prompt();
        let has_model = self
            .document
            .as_ref()
            .and_then(|doc| doc.selected_preset())
            .is_some_and(PresetFile::is_legacy_electric);
        ui.horizontal_wrapped(|ui| {
            let mut enabled = self.live_edit.enabled();
            if ui
                .add_enabled(
                    enabled || free && has_model && self.live_edit.ready(Action::Automatic),
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
            let idle = free && !self.live_edit.busy();
            if ui
                .add_enabled(
                    idle && has_model && self.live_edit.ready(Action::Send),
                    egui::Button::new("Send Edit Buffer"),
                )
                .clicked()
            {
                self.start_edit_buffer(Action::Send, self.live_model());
            }
            if ui
                .add_enabled(
                    idle && self.live_edit.ready(Action::Get),
                    egui::Button::new("Get Edit Buffer"),
                )
                .clicked()
            {
                if self.dirty() {
                    self.pending = Some(super::GuitarPending::Open(
                        super::OpenDestination::EditBuffer,
                    ));
                } else {
                    self.get_edit_buffer();
                }
            }
        });
    }

    pub(super) fn get_edit_buffer(&mut self) {
        if !self.is_reading() && !self.follow.busy() && !self.follow.disconnected() {
            self.start_edit_buffer(Action::Get, self.live_model());
        }
    }

    fn start_edit_buffer(&mut self, action: Action, model: Option<Model>) {
        match self.live_edit.start(action, model) {
            Ok(()) => {
                self.error = None;
                self.status = match action {
                    Action::Get => "Reading guitar working buffer…",
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
        if self
            .live_edit
            .worker
            .as_ref()
            .is_some_and(|work| work.thread.is_finished())
        {
            self.finish_edit_buffer();
        }
        if self.follow.disconnected() {
            self.live_edit.stop();
        }
        if !self.live_edit.enabled()
            || self.is_reading()
            || self.follow.busy()
            || self.opening_file
            || self.blocking_prompt()
        {
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
        if work.cancel.load(Ordering::Relaxed) {
            self.status = "Edit-buffer operation stopped.".into();
            return;
        }
        match (work.action, result) {
            (Action::Automatic | Action::Send, Ok(Reply::Verified(image))) => {
                self.live_edit.verified = work.model.map(|model| (model, image));
                self.status = "Guitar working buffer verified.".into();
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
        Get,
    }

    struct Fake {
        attachment: Attachment,
        calls: Mutex<Vec<Call>>,
        result: PresetFile,
        cancelled: Option<mpsc::Sender<()>>,
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
            assert!(editor.live_edit.start(action, editor.live_model()).is_err());
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

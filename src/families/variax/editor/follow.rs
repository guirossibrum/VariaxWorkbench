use super::{EditorDocument, FamilyTab, VariaxEditor, egui, live_attachment::Attachment};
use crate::editor::tabs::DeviceTab;
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
            FollowUpdate::Clip => self.clip_until = Some(now + CLIP_PULSE),
            FollowUpdate::DspOverload => self.dsp_overload = true,
            FollowUpdate::MidiQueueOverflow => self.queue_overflow = true,
            FollowUpdate::Connected => self.notice = Some(ConnectionNotice::Connected),
            FollowUpdate::Disconnected => self.notice = Some(ConnectionNotice::Disconnected),
        }
    }
}

type Source =
    fn(&Attachment, &AtomicBool, &mut dyn FnMut(FollowUpdate, Instant)) -> Result<(), String>;

struct Worker {
    attachment: Attachment,
    stop: Arc<AtomicBool>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: JoinHandle<Result<(), String>>,
}

#[derive(Default)]
pub(super) struct Follow {
    device_tab: bool,
    attachment: Option<Attachment>,
    worker: Option<Worker>,
    snapshot: Snapshot,
    consumed_program: Option<(Preset, Instant)>,
    notice: Option<ConnectionNotice>,
    error: Option<String>,
}

impl Follow {
    pub(super) fn observe(&mut self, device: Option<&DeviceTab>) {
        self.device_tab = device.is_some();
        let attachment = device.and_then(Attachment::verified);
        if self.attachment != attachment {
            self.stop();
            self.error = None;
            if attachment.is_some() {
                self.notice = None;
            }
            self.attachment = attachment;
        }
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn disconnected(&self) -> bool {
        self.notice == Some(ConnectionNotice::Disconnected)
    }

    pub(super) fn stop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
        }
        self.snapshot = Snapshot::default();
        self.consumed_program = None;
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
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let worker_stop = stop.clone();
        let worker_snapshot = snapshot.clone();
        let thread = std::thread::Builder::new()
            .name("variax-follow".into())
            .spawn(move || {
                let result = source(&worker_attachment, &worker_stop, &mut |update, at| {
                    if let Ok(mut state) = worker_snapshot.lock() {
                        state.receive(update, at);
                    }
                    worker_ctx.request_repaint();
                });
                worker_ctx.request_repaint();
                result
            })
            .map_err(|error| format!("Could not start Variax Follow: {error}"))?;
        self.error = None;
        self.notice = None;
        self.worker = Some(Worker {
            attachment,
            stop,
            snapshot,
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
        let update = worker.snapshot.lock().ok().map(|mut snapshot| {
            let notice = snapshot.notice.take();
            (*snapshot, notice)
        });
        if let Some((snapshot, notice)) = update
            && self.attachment.as_ref() == Some(&worker.attachment)
        {
            match notice {
                Some(ConnectionNotice::Disconnected) => {
                    self.notice = notice;
                    self.stop();
                }
                _ if !stopping => {
                    if notice.is_some() {
                        self.notice = notice;
                    }
                    self.snapshot = snapshot;
                }
                _ => {}
            }
        }

        if finished {
            let worker = self.worker.take().expect("finished Follow worker");
            self.error = worker
                .thread
                .join()
                .unwrap_or_else(|_| Err("The Variax Follow worker stopped unexpectedly.".into()))
                .err();
            self.stop();
        } else {
            // Also repaint on reap if the last worker repaint arrived just
            // before the thread's result became joinable.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
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
        let matches_layout = document.extension() == "vwb"
            && document.banks().is_some_and(|banks| {
                banks.len() == 12 && banks.iter().all(|bank| bank.presets().len() == 5)
            });
        if !matches_layout || document.selection() == Some((bank, slot)) {
            self.consumed_program = Some(program);
            return None;
        }
        let delay = if document.slot_is_dirty(bank, slot) {
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

    fn show(&mut self, ui: &mut egui::Ui, available: bool) {
        if !self.device_tab {
            return;
        }
        let stopping = self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.stop.load(Ordering::Relaxed));
        let mut following = self.busy() && !stopping;
        ui.horizontal_wrapped(|ui| {
            let enabled = following
                || (!stopping
                    && available
                    && self.attachment.is_some()
                    && !self.disconnected()
                    && cfg!(target_os = "linux"));
            if ui
                .add_enabled(enabled, egui::Checkbox::new(&mut following, "Follow"))
                .changed()
            {
                if following {
                    if let Err(error) = self.start(read_updates, ui.ctx()) {
                        self.error = Some(error);
                    }
                } else {
                    self.stop();
                }
            }
            if stopping {
                ui.weak("Stopping Follow…");
            } else if following {
                let snapshot = self.snapshot;
                match snapshot.program {
                    Some((preset, _)) => {
                        ui.label(format!(
                            "Guitar preset · {} {}",
                            preset.bank_name(),
                            preset.slot() + 1
                        ));
                    }
                    None => {
                        ui.weak("Waiting for a guitar preset change");
                    }
                }
                ui.label(control_position("Volume", snapshot.volume));
                ui.label(control_position("Live tone", snapshot.tone));
                let now = Instant::now();
                if let Some(until) = snapshot.clip_until.filter(|until| *until > now) {
                    ui.colored_label(super::FRET_MARKER, "Clip");
                    ui.ctx().request_repaint_after(until - now);
                }
                if snapshot.dsp_overload {
                    ui.colored_label(egui::Color32::LIGHT_RED, "DSP overload");
                }
                if snapshot.queue_overflow {
                    ui.colored_label(super::FRET_MARKER, "MIDI queue overflow");
                }
            }
        });
        match self.notice {
            Some(ConnectionNotice::Connected) if following => {
                ui.weak("Guitar connected via PODxt Live.");
            }
            Some(ConnectionNotice::Disconnected) => {
                ui.weak("Guitar disconnected.");
            }
            _ => {}
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
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
        let available = !self.is_reading() && !self.blocking_prompt();
        self.follow.show(ui, available);
    }

    pub(super) fn poll_follow(&mut self, ctx: &egui::Context) {
        self.follow.listen(ctx);
        if self.is_reading()
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
            }
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn read_updates(
    attachment: &Attachment,
    stop: &AtomicBool,
    publish: &mut dyn FnMut(FollowUpdate, Instant),
) -> Result<(), String> {
    use line6_devices::{
        device_lock::DeviceLock,
        family::variax::follow,
        gate::{GatePolicy, Operation},
        session::{Quiet, Session},
    };
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

    let cancelled = || stop.load(Ordering::Relaxed);
    run_if_unlocked(Gate::OsDiscovery, || Ok(()))?;
    run_if_unlocked(Gate::IdentityRequest, || Ok(()))?;
    run_if_unlocked(Gate::CurrentSettingsRead, || {
        if cancelled() {
            return Ok(());
        }
        let rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
        if cancelled() {
            return Ok(());
        }
        let link = variax_workbench::usb::open_variax_bridge_borrowed(
            &rig,
            (&attachment.path, attachment.address),
        )?;
        let mut log = Quiet;
        let mut session = Session::new(link, &Policy, &mut log);
        session.log_packets = false;
        let result = follow(&rig, &mut session, cancelled, |update| {
            publish(update, Instant::now())
        });
        let release = session.link.finish();
        match (result, release) {
            (Err(error), Err(release)) => Err(format!("{error} {release}")),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    })
}

#[cfg(any(not(target_os = "linux"), test))]
fn read_updates(
    _: &Attachment,
    _: &AtomicBool,
    _: &mut dyn FnMut(FollowUpdate, Instant),
) -> Result<(), String> {
    Err("This build cannot open a USB device for Variax Follow.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tests::app;
    use variax_workbench::{bundle::BundleFile, edit::WIRING};

    fn fake_source(
        attachment: &Attachment,
        stop: &AtomicBool,
        publish: &mut dyn FnMut(FollowUpdate, Instant),
    ) -> Result<(), String> {
        assert_eq!(
            (attachment.path.as_str(), attachment.address),
            ("8-2.1", 10)
        );
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
        Ok(())
    }

    fn poll_until(
        editor: &mut VariaxEditor,
        ctx: &egui::Context,
        predicate: impl Fn(&Follow) -> bool,
    ) {
        let started = Instant::now();
        while !predicate(&editor.follow) {
            editor.poll_follow(ctx);
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
    }

    #[test]
    fn verified_follow_reads_live_values_without_editing_the_document() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        assert!(editor.follow.start(fake_source, &ctx).is_err());
        super::super::refresh::tests::attach(editor);

        let mut document = EditorDocument::from_bundle(
            BundleFile::new_default_bundle().unwrap(),
            "test.vwb".into(),
        )
        .unwrap();
        document.select(4, 2).unwrap();
        document
            .apply(|preset| preset.set_parameter(WIRING, 0x200005, 0, 1))
            .unwrap();
        let before = document.as_bytes().to_vec();
        document.select(0, 0).unwrap();
        editor.document = Some(document);
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
            control_position("Live tone", snapshot.tone),
            "Live tone 100%"
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
    fn completed_worker_delivers_disconnect_before_its_snapshot_is_cleared() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        super::super::refresh::tests::attach(editor);
        editor.change(|preset| preset.rename("Keep this edit"));
        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let source: Source = |_, _, publish| {
            publish(FollowUpdate::Connected, Instant::now());
            publish(FollowUpdate::Disconnected, Instant::now());
            Ok(())
        };

        editor.follow.start(source, &ctx).unwrap();
        let started = Instant::now();
        while !editor.follow.worker.as_ref().unwrap().thread.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
        editor.poll_follow(&ctx);
        assert!(!editor.follow.busy());
        assert!(editor.follow.disconnected());
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
}

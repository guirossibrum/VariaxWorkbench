use super::{
    EditorDocument, FamilyTab, GuitarPending, LibraryOrigin, OpenDestination, VariaxEditor, egui,
    live_attachment::Attachment,
};
use crate::editor::{
    detection::DeviceKey,
    tabs::{Connection, DeviceTab},
};
use line6_devices::family::variax::{self, Restore};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime},
};
use variax_workbench::bundle::BundleFile;

struct Capture {
    bundle: BundleFile,
    restore: Restore,
    release_error: Option<String>,
}

type ReadLibrary = fn(&Attachment, &AtomicBool, &AtomicUsize) -> Result<Capture, String>;

#[derive(Clone, PartialEq, Eq)]
struct Attempt {
    attachment: DeviceKey,
    id: u64,
}

struct DocumentView {
    bytes: Option<Vec<u8>>,
    selection: Option<(usize, usize)>,
    source: Option<PathBuf>,
    history: Option<(bool, bool, bool)>,
}

impl DocumentView {
    fn capture(editor: &VariaxEditor) -> Self {
        Self {
            bytes: editor.document.as_ref().map(|doc| doc.as_bytes().to_vec()),
            selection: editor.document.as_ref().and_then(EditorDocument::selection),
            source: editor.source_path.clone(),
            history: Self::history(editor),
        }
    }

    fn history(editor: &VariaxEditor) -> Option<(bool, bool, bool)> {
        editor
            .document
            .as_ref()
            .map(|doc| (doc.is_dirty(), doc.can_undo(), doc.can_redo()))
    }

    fn still_open(&self, editor: &VariaxEditor) -> bool {
        self.bytes.as_deref() == editor.document.as_ref().map(EditorDocument::as_bytes)
            && self.selection == editor.document.as_ref().and_then(EditorDocument::selection)
            && self.source == editor.source_path
            && self.history == Self::history(editor)
    }
}

struct DownloadedLibrary {
    bundle: BundleFile,
    position: (usize, usize),
    captured_at: SystemTime,
    attachment: DeviceKey,
}

impl DownloadedLibrary {
    fn document(&self) -> Result<EditorDocument, String> {
        let mut document =
            EditorDocument::from_bundle(self.bundle.clone(), "Downloaded Variax library".into())?;
        document.select(self.position.0, self.position.1)?;
        Ok(document)
    }

    fn origin(&self) -> LibraryOrigin {
        LibraryOrigin::AttachedDownload(self.captured_at, self.attachment.clone())
    }

    fn is_open(&self, origin: Option<&LibraryOrigin>) -> bool {
        matches!(origin, Some(LibraryOrigin::AttachedDownload(time, key))
            if *time == self.captured_at && *key == self.attachment)
    }
}

struct Worker {
    attempt: Attempt,
    replace: Option<DocumentView>,
    stop: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    thread: JoinHandle<Result<Capture, String>>,
}

#[derive(Default)]
pub(super) struct Refresh {
    attachment: Option<Attachment>,
    attempted: Option<DeviceKey>,
    active: Option<Attempt>,
    next_attempt: u64,
    latest_download: Option<DownloadedLibrary>,
    explicit_download_required: bool,
    #[cfg(test)]
    auto_source: Option<ReadLibrary>,
    confirming: bool,
    worker: Option<Worker>,
}

impl Refresh {
    pub(super) fn observe(&mut self, device: Option<&DeviceTab>) {
        let attachment = device.and_then(Attachment::verified);
        if device.is_none_or(|tab| tab.connection == Connection::Disconnected) {
            self.attempted = None;
        }
        if self.attachment != attachment {
            self.cancel();
            self.active = None;
            self.confirming = false;
            self.attachment = attachment;
        }
    }

    pub(super) fn require_explicit_download(&mut self) {
        // A historical library previously suppressed automatic downloads.
        // Keep that behavior when it is skipped during device-tab startup.
        self.explicit_download_required = true;
    }

    pub(super) fn keep_open_document(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.replace = None;
        }
    }

    fn accepts(&self, attempt: &Attempt) -> bool {
        self.active.as_ref() == Some(attempt)
            && self.attachment.as_ref().map(|a| &a.key) == Some(&attempt.attachment)
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn confirming(&self) -> bool {
        self.confirming
    }

    pub(super) fn cancel(&self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
        }
    }

    fn start(&mut self, read: ReadLibrary, replace: Option<DocumentView>) -> Result<(), String> {
        let attachment = self
            .attachment
            .clone()
            .ok_or("The verified Variax attachment is no longer available.")?;
        self.next_attempt += 1;
        let attempt = Attempt {
            attachment: attachment.key.clone(),
            id: self.next_attempt,
        };
        self.attempted = Some(attachment.key.clone());
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let worker_stop = stop.clone();
        let worker_progress = progress.clone();
        let thread = std::thread::Builder::new()
            .name("variax-refresh".into())
            .spawn(move || read(&attachment, &worker_stop, &worker_progress))
            .map_err(|error| format!("Could not start Variax Refresh: {error}"))?;
        self.active = Some(attempt.clone());
        self.worker = Some(Worker {
            attempt,
            replace,
            stop,
            progress,
            thread,
        });
        Ok(())
    }
}

impl Drop for Refresh {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.thread.join();
        }
    }
}

impl VariaxEditor {
    pub(super) fn connected_library_attachment(&self) -> Option<&DeviceKey> {
        self.refresh
            .attachment
            .as_ref()
            .filter(|_| !self.follow.disconnected())
            .map(|attachment| &attachment.key)
    }

    pub(super) fn download_target_ready(&self) -> bool {
        self.refresh.attachment.is_some()
            && !self.follow.disconnected()
            && !self.device_sync.stopped_after_store()
    }

    pub(super) fn download_button(&mut self, ui: &mut egui::Ui) {
        let button = ui.add_enabled(
            cfg!(target_os = "linux") && self.download_target_ready() && !self.refresh.busy(),
            egui::Button::new("Download from guitar…")
                .min_size(egui::vec2(0.0, super::theme::TARGET)),
        );
        if button.clicked() {
            self.request_refresh(read_library);
        }
    }

    fn request_refresh(&mut self, read: ReadLibrary) {
        if self.is_reading()
            || self.opening_file
            || self.live_edit.busy()
            || self.follow.disconnected()
            || self.blocking_prompt()
            || !self.download_target_ready()
        {
            return;
        }
        if FamilyTab::dirty(self) {
            self.refresh.confirming = true;
        } else {
            self.start_refresh(read, self.untouched_startup_view());
        }
    }

    fn untouched_startup_view(&self) -> bool {
        !FamilyTab::dirty(self)
            && !self.opening_file
            && self.source_path.is_none()
            && self.parked_library.is_none()
            && self.name_draft.is_none()
            && self.author_draft.is_none()
            && self.document.as_ref().is_none_or(|document| {
                matches!(self.library_origin, Some(LibraryOrigin::Factory))
                    && !document.can_undo()
                    && !document.can_redo()
            })
    }

    fn start_refresh(&mut self, read: ReadLibrary, replace: bool) {
        let replace = (replace
            && !self.opening_file
            && self.name_draft.is_none()
            && self.author_draft.is_none())
        .then(|| DocumentView::capture(self));
        match self.refresh.start(read, replace) {
            Ok(()) => {
                self.error = None;
                self.status = "Reading guitar library…".into();
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn refresh_confirmation(&mut self, ctx: &egui::Context) {
        if !self.refresh.confirming {
            return;
        }
        let dialog = egui::Modal::new(egui::Id::new("variax-refresh-confirmation")).show(ctx, |ui| {
            ui.heading("Replace unsaved changes?");
            ui.label("Download reads the stored presets and bank names. Choose whether to keep this document or replace it after a successful download.");
            ui.horizontal(|ui| {
                let cancel = ui.button("Keep editing");
                let download = ui.button("Download and keep current document");
                let replace = ui.button("Download and replace");
                (cancel.clicked(), download.clicked(), replace.clicked())
            }).inner
        });
        let (cancel, download, replace) = dialog.inner;
        if download {
            self.refresh.confirming = false;
            self.start_refresh(read_library, false);
        } else if cancel || replace || dialog.should_close() {
            self.confirm_refresh(replace, read_library);
        }
    }

    fn confirm_refresh(&mut self, replace: bool, read: ReadLibrary) {
        if std::mem::take(&mut self.refresh.confirming) && replace {
            self.start_refresh(read, true);
        }
    }

    pub(super) fn start_automatic_download(&mut self) {
        #[cfg(test)]
        let Some(read) = self.refresh.auto_source else {
            return;
        };
        #[cfg(not(test))]
        let read = read_library;
        if !cfg!(target_os = "linux")
            || self.refresh.explicit_download_required
            || !self.download_target_ready()
            || self.is_reading()
            || self.live_edit.busy()
            || self.blocking_prompt()
            || !self.untouched_startup_view()
            || self
                .refresh
                .attachment
                .as_ref()
                .is_none_or(|attachment| self.refresh.attempted.as_ref() == Some(&attachment.key))
        {
            return;
        }
        // Workbench 1.75 refreshed a missing bundle on connect. The editor
        // here starts only the library read and does not send an edit buffer.
        self.start_refresh(read, true);
    }

    pub(super) fn poll_refresh(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.refresh.worker else {
            return;
        };
        if !worker.thread.is_finished() {
            if self.refresh.accepts(&worker.attempt) {
                if worker.stop.load(Ordering::Relaxed) {
                    self.status = "Stopping guitar refresh and releasing the bridge…".into();
                } else {
                    let completed = worker.progress.load(Ordering::Relaxed);
                    let total = variax::Preset::all().count();
                    self.status = format!("Reading guitar library · {completed}/{total} presets");
                }
            }
            ctx.request_repaint_after(Duration::from_millis(80));
            return;
        }
        let worker = self.refresh.worker.take().expect("finished worker");
        let result = worker
            .thread
            .join()
            .unwrap_or_else(|_| Err("The Variax Refresh worker stopped unexpectedly.".into()));
        self.finish_refresh(
            worker.attempt,
            worker.stop.load(Ordering::Relaxed),
            worker.replace,
            result,
        );
    }

    fn finish_refresh(
        &mut self,
        attempt: Attempt,
        cancelled: bool,
        replace: Option<DocumentView>,
        result: Result<Capture, String>,
    ) {
        if !self.refresh.accepts(&attempt) {
            return;
        }
        self.refresh.active = None;
        if self.follow.disconnected() {
            return;
        }
        if cancelled {
            self.status = "Guitar refresh stopped. The open document was kept.".into();
            self.error = result.err();
            return;
        }

        let result = result.and_then(|capture| {
            let Restore::Switched { to, verified: true } = capture.restore else {
                return Err(with_release_error(
                    restore_notice(&capture.restore),
                    capture.release_error,
                ));
            };
            if let Some(error) = capture.release_error {
                return Err(error);
            }
            let download = DownloadedLibrary {
                bundle: capture.bundle,
                position: (usize::from(to.bank()), usize::from(to.slot())),
                captured_at: SystemTime::now(),
                attachment: attempt.attachment,
            };
            let document = download.document()?;
            Ok((download, document))
        });
        let (download, document) = match result {
            Ok(completed) => completed,
            Err(error) => {
                self.error = Some(error);
                self.status = "Guitar refresh did not complete. The open document was kept.".into();
                return;
            }
        };

        let snapshot_error = self.preferences_path.as_deref().and_then(|preferences| {
            super::startup_library::save_download(preferences, &document, download.captured_at)
                .err()
                .map(|error| format!("Could not remember the downloaded Variax library: {error}"))
        });
        let origin = download.origin();
        self.refresh.latest_download = Some(download);
        self.refresh.explicit_download_required = false;

        if replace.is_some_and(|view| view.still_open(self))
            && !self.opening_file
            && !self.blocking_prompt()
            && self.parked_library.is_none()
            && self.name_draft.is_none()
            && self.author_draft.is_none()
        {
            self.adopt_document(document, None);
            self.library_origin = Some(origin);
            self.status =
                "Read 60 guitar presets and 12 bank names. Use Save As to keep this library."
                    .into();
        } else {
            self.status =
                "Read 60 guitar presets and 12 bank names. The download is ready to open.".into();
        }
        self.error = snapshot_error;
    }

    pub(super) fn open_download_button(&mut self, ui: &mut egui::Ui) {
        if self
            .refresh
            .latest_download
            .as_ref()
            .is_none_or(|download| download.is_open(self.library_origin.as_ref()))
        {
            return;
        }
        let available = !self.is_reading()
            && !self.blocking_prompt()
            && !self.opening_file
            && self.name_draft.is_none()
            && self.author_draft.is_none()
            && self
                .parked_library
                .as_ref()
                .is_none_or(|library| !library.document.is_dirty());
        if ui
            .add_enabled(available, egui::Button::new("Open downloaded library…"))
            .clicked()
        {
            if FamilyTab::dirty(self) {
                self.pending = Some(GuitarPending::Open(OpenDestination::Download));
            } else {
                self.open_download();
            }
        }
    }

    pub(super) fn open_download(&mut self) {
        if let Some(download) = &self.refresh.latest_download {
            match download.document() {
                Ok(document) => {
                    let origin = download.origin();
                    self.adopt_document(document, None);
                    self.library_origin = Some(origin);
                    self.status = "Opened the downloaded Variax library.".into();
                }
                Err(error) => self.error = Some(error),
            }
        }
    }
}

fn restore_notice(restore: &Restore) -> String {
    match restore {
        Restore::Switched { to, verified } => {
            let preset = format!("{} {}", to.bank_name(), to.slot() + 1);
            if *verified {
                format!("Reselected and verified {preset}.")
            } else {
                format!("Reselected {preset}, but read-back did not verify it.")
            }
        }
        Restore::NoUniqueMatch { matches } => format!(
            "Found {matches} matches for the original guitar preset. No preset was reselected."
        ),
        Restore::Failed(error) => format!("Could not restore the original guitar preset: {error}"),
    }
}

fn with_release_error(message: String, release_error: Option<String>) -> String {
    match release_error {
        Some(error) => format!("{message} {error}"),
        None => message,
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn read_library(
    attachment: &Attachment,
    stop: &AtomicBool,
    progress: &AtomicUsize,
) -> Result<Capture, String> {
    use line6_devices::{
        device_lock::DeviceLock,
        family::variax::backup::refresh_library,
        gate::{GatePolicy, Operation},
        session::{Quiet, Session},
    };
    use variax_workbench::hardware_gates::{Gate, is_locked, run_if_unlocked};

    struct Policy;
    impl GatePolicy for Policy {
        fn allows(&self, operation: Operation) -> bool {
            let gate = match operation {
                Operation::VariaxIdentity => Gate::IdentityRequest,
                Operation::VariaxRead => Gate::CurrentSettingsRead,
                Operation::VariaxLibraryBridge | Operation::VariaxPresetSwitch => {
                    Gate::FullBackupRead
                }
                Operation::VariaxBankNameRead => Gate::StoreRestore,
                _ => return false,
            };
            !is_locked(gate)
        }
    }

    let cancelled = || stop.load(Ordering::Relaxed);
    for gate in [
        Gate::OsDiscovery,
        Gate::IdentityRequest,
        Gate::CurrentSettingsRead,
        Gate::StoreRestore,
    ] {
        run_if_unlocked(gate, || Ok(()))?;
    }
    run_if_unlocked(Gate::FullBackupRead, || {
        if cancelled() {
            return Err("Guitar refresh cancelled before opening the attachment.".into());
        }
        let rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
        if cancelled() {
            return Err("Guitar refresh cancelled before opening the attachment.".into());
        }
        let link = variax_workbench::usb::open_variax_bridge_borrowed(
            &rig,
            (&attachment.path, attachment.address),
        )?;
        let mut log = Quiet;
        let mut session = Session::new(link, &Policy, &mut log);
        session.log_packets = false;
        let result = refresh_library(&rig, &mut session, cancelled, |_| {
            progress.fetch_add(1, Ordering::Relaxed);
        });
        let cleanup = session.link.finish().err();
        let refreshed =
            result.map_err(|error| with_release_error(error.to_string(), cleanup.clone()))?;
        let release_error = match (refreshed.release_error, cleanup) {
            (Some(message), cleanup) => Some(with_release_error(message, cleanup)),
            (None, cleanup) => cleanup,
        };
        let bundle =
            BundleFile::from_refreshed_variax(refreshed.library, &refreshed.identified.reply)
                .map_err(|error| with_release_error(error, release_error.clone()))?;
        if cancelled() {
            return Err(with_release_error(
                restore_notice(&refreshed.restore),
                release_error,
            ));
        }
        Ok(Capture {
            bundle,
            restore: refreshed.restore,
            release_error,
        })
    })
}

#[cfg(any(not(target_os = "linux"), test))]
fn read_library(_: &Attachment, _: &AtomicBool, _: &AtomicUsize) -> Result<Capture, String> {
    Err("This build cannot open a USB device for Variax Refresh.".into())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::editor::{Scan, detection, tests::app};
    use variax_workbench::device_probe::{OsMetadataSource, PODXT_LIVE_USB, SyntheticSysfs};

    struct VerifiedRig(SyntheticSysfs);

    impl detection::PresenceSource for VerifiedRig {
        fn metadata(&self) -> &dyn OsMetadataSource {
            &self.0
        }

        fn usb_address(&self, _: &str) -> Option<u8> {
            Some(10)
        }

        fn simulated_identity(
            &self,
            _: &detection::SimulatedJob,
        ) -> Option<detection::SimulatedOutcome> {
            Some(detection::SimulatedOutcome::Finished {
                pod_firmware: Some("3.01"),
                guitar: detection::SimulatedGuitar::Verified("3.10"),
            })
        }
    }

    pub(crate) fn verified_rig() -> detection::DetectionService {
        let mut metadata = SyntheticSysfs::new();
        metadata.add_device(
            "8-2.1",
            PODXT_LIVE_USB.0,
            PODXT_LIVE_USB.1,
            "PODxt Live",
            &[],
        );
        detection::DetectionService::new(Box::new(VerifiedRig(metadata)))
    }

    pub(crate) fn verified_tab() -> DeviceTab {
        verified_tab_for(&mut verified_rig())
    }

    pub(crate) fn verified_tab_for(detection: &mut detection::DetectionService) -> DeviceTab {
        let devices = detection
            .refresh(Scan::Automatic, crate::editor::registry::DEVICE_PROFILES)
            .unwrap();
        let mut tabs = crate::editor::Tabs::default();
        tabs.reconcile_devices(
            devices,
            crate::editor::registry::FAMILIES,
            &crate::editor::EditorDefaults {
                artwork: Default::default(),
                preferences: None,
                startup_panel: None,
            },
        )
        .unwrap();
        tabs.entries
            .into_iter()
            .find_map(|tab| {
                tab.device
                    .filter(|device| device.device.key.family_id == "variax")
            })
            .unwrap()
    }

    pub(crate) fn attach(editor: &mut VariaxEditor) {
        editor.observe_device(Some(&verified_tab()));
    }

    pub(in crate::editor::variax_panel) fn allow_fake_automatic_download(
        editor: &mut VariaxEditor,
    ) {
        editor.refresh.auto_source = Some(complete_capture);
    }

    fn complete_capture(
        attachment: &Attachment,
        _: &AtomicBool,
        progress: &AtomicUsize,
    ) -> Result<Capture, String> {
        assert_eq!(
            (attachment.path.as_str(), attachment.address),
            ("8-2.1", 10)
        );
        progress.store(60, Ordering::Relaxed);
        Ok(Capture {
            bundle: BundleFile::new_default_bundle()?,
            restore: Restore::Switched {
                to: variax::Preset::new(3, 2).unwrap(),
                verified: true,
            },
            release_error: None,
        })
    }

    fn wait_for_worker(editor: &VariaxEditor) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !editor.refresh.worker.as_ref().unwrap().thread.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "fake Refresh worker did not finish"
            );
            std::thread::yield_now();
        }
    }

    fn wait_for_completion(editor: &mut VariaxEditor, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while editor.refresh.busy() {
            editor.poll_refresh(ctx);
            assert!(
                std::time::Instant::now() < deadline,
                "fake refresh did not finish"
            );
            std::thread::yield_now();
        }
    }

    pub(crate) fn download(editor: &mut VariaxEditor, ctx: &egui::Context) {
        editor.request_refresh(complete_capture);
        editor.confirm_refresh(true, complete_capture);
        wait_for_completion(editor, ctx);
        if !editor.has_current_guitar_library() {
            editor.open_download();
        }
        assert!(editor.has_current_guitar_library());
    }

    #[test]
    fn a_verified_connection_downloads_once_and_cancel_remains_available() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.document = None;
        editor.source_path = None;
        editor.refresh.auto_source = Some(|attachment, stop, progress| {
            progress.store(19, Ordering::Relaxed);
            while !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            complete_capture(attachment, stop, progress)
        });
        editor.poll(&ctx);
        assert!(!editor.refresh.busy());

        attach(editor);
        editor.poll(&ctx);
        assert!(editor.refresh.busy());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !editor.status.contains("19/60") {
            editor.poll(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        editor.cancel_operation();
        wait_for_completion(editor, &ctx);
        assert!(editor.document.is_none());
        editor.poll(&ctx);
        assert!(
            !editor.refresh.busy(),
            "cancel should persist for this connection"
        );

        editor.observe_device(None);
        attach(editor);
        editor.refresh.auto_source = Some(complete_capture);
        editor.poll(&ctx);
        wait_for_completion(editor, &ctx);
        assert!(matches!(
            editor.library_origin,
            Some(super::super::LibraryOrigin::AttachedDownload(..))
        ));
        assert_eq!(editor.document.as_ref().unwrap().banks().unwrap().len(), 12);
    }

    #[test]
    fn download_provenance_expires_when_the_guitar_on_the_same_bridge_changes() {
        let (ctx, _) = app();
        let directory = tempfile::tempdir().unwrap();
        let defaults = super::super::EditorDefaults {
            artwork: directory.path().join("artwork"),
            preferences: Some(directory.path().join("editor-preferences.txt")),
            startup_panel: None,
        };
        let mut rig = verified_rig();
        let first = verified_tab_for(&mut rig);
        let mut editor = VariaxEditor::new(&defaults);
        editor.observe_device(Some(&first));
        editor.request_refresh(complete_capture);
        wait_for_completion(&mut editor, &ctx);
        let downloaded = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let connected_label = editor.library_label().unwrap();
        assert!(connected_label.starts_with("Downloaded on this connection · "));
        assert!(editor.has_current_guitar_library());

        // The same bridge and model/firmware answer after a guitar disconnect.
        let profiles = crate::editor::registry::DEVICE_PROFILES;
        rig.report_guitar_disconnect(&first.device.key, profiles)
            .unwrap();
        assert!(rig.queue_guitar_refresh(&first.device.key.attachment, false));
        let second = verified_tab_for(&mut rig);
        assert_eq!(first.device.key.attachment, second.device.key.attachment);
        assert_eq!(first.device.support, second.device.support);
        assert_ne!(first.device.key, second.device.key);

        editor.change(|preset| preset.rename("Keep this edit"));
        let edited = editor.document.as_ref().unwrap().as_bytes().to_vec();
        editor.observe_device(Some(&second));
        let historical = editor.library_label().unwrap();
        assert!(historical.starts_with("Saved guitar download · "));
        assert!(historical.ends_with("Source guitar unverified for this connection"));
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), edited);
        assert!(editor.dirty());
        assert!(!editor.has_current_guitar_library());
        assert!(!editor.sync_ready(super::super::device_sync::Action::ReadUnmodified));
        assert!(editor.download_target_ready());
        editor.refresh.auto_source = Some(complete_capture);
        editor.poll(&ctx);
        assert!(!editor.refresh.busy());

        editor.undo();
        editor.poll(&ctx);
        assert!(!editor.refresh.busy());
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), downloaded);

        let mut reopened = VariaxEditor::new(&defaults);
        reopened.load_initial_library(&defaults);
        assert_eq!(reopened.document.as_ref().unwrap().as_bytes(), downloaded);
        assert!(
            reopened
                .library_label()
                .unwrap()
                .starts_with("Saved guitar download · ")
        );
        assert!(
            !reopened
                .library_label()
                .unwrap()
                .contains("this connection")
        );
        reopened.observe_device(Some(&first));
        assert_eq!(reopened.library_label(), Some(historical));
        assert!(reopened.has_historical_guitar_library());
        assert!(!reopened.sync_ready(super::super::device_sync::Action::ReadUnmodified));

        editor.request_refresh(complete_capture);
        wait_for_completion(&mut editor, &ctx);
        assert!(!editor.has_current_guitar_library());
        editor.open_download();
        assert!(editor.has_current_guitar_library());
        if cfg!(target_os = "linux") {
            assert!(editor.sync_ready(super::super::device_sync::Action::ReadUnmodified));
        }
        assert!(
            editor
                .library_label()
                .unwrap()
                .starts_with("Downloaded on this connection · ")
        );
    }

    #[test]
    fn automatic_download_respects_saved_libraries_files_and_dirty_factory_edits() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.refresh.auto_source = Some(complete_capture);
        attach(editor);
        editor.poll(&ctx);
        assert!(
            !editor.refresh.busy(),
            "an explicitly opened file stays open"
        );

        editor.source_path = None;
        editor.library_origin = Some(super::super::LibraryOrigin::Download(SystemTime::now()));
        editor.poll(&ctx);
        assert!(
            !editor.refresh.busy(),
            "a saved download already supplies the library"
        );

        editor.library_origin = Some(super::super::LibraryOrigin::Backup(
            variax_workbench::variax_library::BackupSource::ImportedBackup,
            SystemTime::now(),
        ));
        editor.poll(&ctx);
        assert!(
            !editor.refresh.busy(),
            "an import already supplies the library"
        );

        editor.library_origin = Some(super::super::LibraryOrigin::Factory);
        editor.change(|preset| preset.rename("Keep these edits"));
        editor.poll(&ctx);
        assert!(!editor.refresh.busy());
        editor.undo();
        editor.poll(&ctx);
        assert!(
            !editor.refresh.busy(),
            "keep the factory document's redo history"
        );

        editor
            .adopt_startup_library(
                BundleFile::new_default_bundle().unwrap(),
                LibraryOrigin::Factory,
            )
            .unwrap();
        editor.poll(&ctx);
        wait_for_completion(editor, &ctx);
        assert!(matches!(
            editor.library_origin,
            Some(super::super::LibraryOrigin::AttachedDownload(..))
        ));
    }

    #[test]
    fn verified_refresh_confirms_dirty_replacement_and_loads_a_clean_bundle() {
        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let defaults = super::super::EditorDefaults {
            artwork: directory.path().join("artwork"),
            preferences: Some(directory.path().join("editor-preferences.txt")),
            startup_panel: None,
        };
        let editor = app.variax_mut();
        editor.preferences_path.clone_from(&defaults.preferences);
        editor.change(|preset| preset.rename("Unsaved edit"));
        let old = editor.document.as_ref().unwrap().as_bytes().to_vec();
        editor.request_refresh(complete_capture);
        assert!(!editor.refresh.busy() && !editor.refresh.confirming());

        attach(editor);
        editor.request_refresh(complete_capture);
        assert!(editor.blocking_prompt() && !editor.refresh.busy());
        editor.confirm_refresh(false, complete_capture);
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), old);

        editor.request_refresh(complete_capture);
        editor.confirm_refresh(true, complete_capture);
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), old);
        wait_for_completion(editor, &ctx);

        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((3, 2)));
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        assert!(!document.slot_is_dirty(3, 2));
        assert!(editor.source_path().is_none());

        let mut reopened = VariaxEditor::new(&defaults);
        reopened.load_initial_library(&defaults);
        assert_eq!(
            reopened.document.as_ref().unwrap().as_bytes(),
            document.as_bytes()
        );
        assert!(matches!(
            reopened.library_origin,
            Some(super::super::LibraryOrigin::Download(_))
        ));
        assert!(!reopened.dirty());

        let target = directory.path().join("Guitar.vwb");
        std::fs::write(&target, b"original").unwrap();
        editor.change(|preset| preset.rename("Edit after reading"));
        assert!(editor.dirty());
        assert!(!editor.save_copy_to(target.clone()));
        assert!(editor.dirty());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        let new = directory.path().join("Copy.vwb");
        assert!(editor.save_copy_to(new.clone()));
        assert!(!editor.dirty());
        assert_eq!(
            EditorDocument::open(&new).unwrap().banks().unwrap().len(),
            12
        );
    }

    #[test]
    fn download_retains_a_complete_library_while_an_open_file_stays_in_use() {
        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let preferences = directory.path().join("editor-preferences.txt");
        let file = directory.path().join("Open guitar.vwb");
        let mut original =
            EditorDocument::from_bundle(BundleFile::new_default_bundle().unwrap(), file.clone())
                .unwrap();
        original.apply(|preset| preset.rename("My file")).unwrap();
        std::fs::write(&file, original.as_bytes()).unwrap();

        let editor = app.variax_mut();
        editor.preferences_path = Some(preferences.clone());
        editor.load(&file);
        attach(editor);
        editor.request_refresh(complete_capture);
        wait_for_completion(editor, &ctx);
        assert_eq!(editor.source_path(), Some(file.as_path()));
        assert_eq!(
            editor.document.as_ref().unwrap().as_bytes(),
            original.as_bytes()
        );
        assert_eq!(std::fs::read(&file).unwrap(), original.as_bytes());
        assert!(!editor.has_current_guitar_library());

        let retained = editor.refresh.latest_download.as_ref().unwrap();
        let bytes = retained.document().unwrap().as_bytes().to_vec();
        let defaults = super::super::EditorDefaults {
            artwork: directory.path().join("artwork"),
            preferences: Some(preferences),
            startup_panel: None,
        };
        let mut offline = VariaxEditor::new(&defaults);
        offline.load_initial_library(&defaults);
        assert_eq!(offline.document.as_ref().unwrap().as_bytes(), bytes);
        editor.open_download();
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), bytes);
        assert!(editor.has_current_guitar_library());
    }

    #[test]
    fn a_snapshot_save_failure_keeps_the_successful_download_available() {
        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let preferences = directory.path().join("editor-preferences.txt");
        std::fs::write(directory.path().join("variax-library-snapshots"), b"keep").unwrap();
        let editor = app.variax_mut();
        editor.preferences_path = Some(preferences);
        attach(editor);

        editor.request_refresh(complete_capture);
        wait_for_completion(editor, &ctx);
        assert!(editor.status.starts_with("Read 60 guitar presets"));
        assert!(
            editor
                .error
                .as_deref()
                .unwrap()
                .contains("Could not remember")
        );
        assert_eq!(
            std::fs::read(directory.path().join("variax-library-snapshots")).unwrap(),
            b"keep",
        );
        assert!(!editor.has_current_guitar_library());
        editor.open_download();
        assert!(editor.has_current_guitar_library());
    }

    #[test]
    fn completion_keeps_a_late_file_open_even_when_the_document_bytes_match() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        attach(editor);
        editor.source_path = None;
        editor.library_origin = Some(LibraryOrigin::Factory);
        let bytes = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let reopened =
            EditorDocument::from_file_bytes(bytes.clone(), std::path::Path::new("same.vwb"))
                .unwrap();

        editor.start_refresh(complete_capture, true);
        // Simulate delivery of another document while the worker is active.
        editor.adopt_opened_document(reopened, None);
        wait_for_completion(editor, &ctx);
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), bytes);
        assert!(editor.library_origin.is_none());
        assert!(editor.refresh.latest_download.is_some());
    }

    #[test]
    fn completion_keeps_edits_and_drafts_added_after_replacement_was_confirmed() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        attach(editor);
        for late_edit in [true, false] {
            editor.change(|preset| preset.rename("Confirmed edit"));
            editor.request_refresh(complete_capture);
            editor.confirm_refresh(true, complete_capture);
            if late_edit {
                // Synthetic editor update between authorization and result delivery.
                editor
                    .document
                    .as_mut()
                    .unwrap()
                    .apply(|preset| preset.rename("Later edit"))
                    .unwrap();
            } else {
                editor.name_draft = Some("Unfinished name".into());
            }
            let bytes = editor.document.as_ref().unwrap().as_bytes().to_vec();
            wait_for_completion(editor, &ctx);
            let document = editor.document.as_ref().unwrap();
            assert_eq!(document.as_bytes(), bytes);
            assert!(document.is_dirty() && document.can_undo());
            assert_eq!(
                editor.name_draft.as_deref(),
                (!late_edit).then_some("Unfinished name")
            );
            assert!(editor.refresh.latest_download.is_some());
            assert!(!editor.has_current_guitar_library());
        }
    }

    #[test]
    fn queued_follow_disconnect_wins_over_refresh_completion_in_the_same_poll() {
        use super::super::new_files::synthetic_backup;
        use crate::editor::tabs::DeviceEvent;

        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let preferences = directory.path().join("editor-preferences.txt");
        let snapshots = preferences.with_file_name("variax-library-snapshots");
        let editor = app.variax_mut();
        editor.preferences_path = Some(preferences);
        attach(editor);
        editor.start_refresh(complete_capture, true);
        wait_for_completion(editor, &ctx);
        let prior = editor.refresh.latest_download.as_ref().unwrap();
        let prior_time = prior.captured_at;
        let prior_bytes = prior.bundle.as_bytes().to_vec();
        let before = synthetic_backup::snapshot(&snapshots);
        assert_eq!(before.len(), 1);

        editor.change(|preset| preset.rename("Keep this edit"));
        let document = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let next_capture: ReadLibrary = |attachment, stop, progress| {
            let mut capture = complete_capture(attachment, stop, progress)?;
            let mut library = EditorDocument::from_bundle(capture.bundle, PathBuf::new())?;
            library.apply(|preset| preset.rename("Obsolete read"))?;
            capture.bundle = BundleFile::parse(library.as_bytes().to_vec())?;
            Ok(capture)
        };
        editor.request_refresh(next_capture);
        editor.confirm_refresh(true, next_capture);
        wait_for_worker(editor);
        editor.follow.queue_disconnect(&ctx);
        assert!(!editor.follow.disconnected());

        FamilyTab::poll(editor, &ctx);

        assert!(editor.follow.disconnected());
        assert!(matches!(
            editor.take_device_event(),
            Some(DeviceEvent::GuitarDisconnected(key))
                if key == editor.refresh.attachment.as_ref().unwrap().key
        ));
        assert!(!editor.refresh.busy());
        assert!(editor.refresh.active.is_none());
        let retained = editor.refresh.latest_download.as_ref().unwrap();
        assert!(
            retained.bundle.as_bytes() == prior_bytes,
            "a disconnected result replaced the retained library"
        );
        assert_eq!(retained.captured_at, prior_time);
        let current = editor.document.as_ref().unwrap();
        assert!(
            current.as_bytes() == document,
            "the edited document changed"
        );
        assert!(current.is_dirty() && current.can_undo());
        assert!(matches!(
            editor.library_origin.as_ref(),
            Some(LibraryOrigin::AttachedDownload(time, key))
                if *time == prior_time && *key == retained.attachment
        ));
        assert!(
            synthetic_backup::snapshot(&snapshots) == before,
            "the library snapshot directory changed"
        );
    }

    #[test]
    fn obsolete_attachment_and_attempt_cannot_publish_or_replace_a_library() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        let mut rig = verified_rig();
        let first = verified_tab_for(&mut rig);
        editor.observe_device(Some(&first));
        let capture = || {
            complete_capture(
                &Attachment::verified(&first).unwrap(),
                &AtomicBool::new(false),
                &AtomicUsize::new(0),
            )
        };

        editor.start_refresh(complete_capture, false);
        let old = editor.refresh.worker.as_ref().unwrap().attempt.clone();
        wait_for_completion(editor, &ctx);
        let previous = editor.refresh.latest_download.as_ref().unwrap().captured_at;

        editor.start_refresh(complete_capture, true);
        let current = editor.refresh.worker.as_ref().unwrap().attempt.clone();
        let view = DocumentView::capture(editor);
        editor.finish_refresh(old, false, Some(view), capture());
        assert!(editor.refresh.accepts(&current));
        assert_eq!(
            editor.refresh.latest_download.as_ref().unwrap().captured_at,
            previous
        );

        let profiles = crate::editor::registry::DEVICE_PROFILES;
        rig.report_guitar_disconnect(&first.device.key, profiles)
            .unwrap();
        assert!(rig.queue_guitar_refresh(&first.device.key.attachment, false));
        let second = verified_tab_for(&mut rig);
        editor.observe_device(Some(&second));
        assert_eq!(first.device.key.attachment, second.device.key.attachment);
        assert_ne!(first.device.key, second.device.key);
        let bytes = editor.document.as_ref().unwrap().as_bytes().to_vec();
        wait_for_completion(editor, &ctx);
        editor.finish_refresh(current, false, None, capture());
        assert_eq!(
            editor.refresh.latest_download.as_ref().unwrap().captured_at,
            previous
        );
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), bytes);
        assert!(!editor.has_current_guitar_library());

        editor.start_refresh(complete_capture, false);
        wait_for_completion(editor, &ctx);
        assert_eq!(
            editor.refresh.latest_download.as_ref().unwrap().attachment,
            second.device.key
        );
    }

    #[test]
    fn final_bank_name_return_release_and_late_cancellation_keep_the_last_complete_library() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        attach(editor);
        editor.start_refresh(complete_capture, false);
        wait_for_completion(editor, &ctx);
        let previous = editor.refresh.latest_download.as_ref().unwrap().captured_at;
        let failures: [ReadLibrary; 3] = [
            |_, _, progress| {
                progress.store(60, Ordering::Relaxed);
                Err("Missing the final bank name.".into())
            },
            |attachment, stop, progress| {
                let mut capture = complete_capture(attachment, stop, progress)?;
                capture.restore = Restore::NoUniqueMatch { matches: 2 };
                Ok(capture)
            },
            |attachment, stop, progress| {
                let mut capture = complete_capture(attachment, stop, progress)?;
                capture.release_error = Some("Failed to release the bridge.".into());
                Ok(capture)
            },
        ];
        for (read, expected) in failures.into_iter().zip([
            "Missing the final bank name",
            "Found 2 matches",
            "Failed to release the bridge",
        ]) {
            editor.start_refresh(read, true);
            wait_for_completion(editor, &ctx);
            assert!(editor.error.as_deref().unwrap().contains(expected));
            assert_eq!(
                editor.refresh.latest_download.as_ref().unwrap().captured_at,
                previous
            );
        }

        editor.start_refresh(complete_capture, true);
        wait_for_worker(editor);
        editor.cancel_operation();
        wait_for_completion(editor, &ctx);
        assert!(editor.status.starts_with("Guitar refresh stopped."));
        assert_eq!(
            editor.refresh.latest_download.as_ref().unwrap().captured_at,
            previous
        );
        assert!(!editor.has_current_guitar_library());
    }

    #[test]
    fn a_failed_or_cancelled_refresh_keeps_the_existing_document_and_history() {
        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.change(|preset| preset.rename("Keep this"));
        let prior = editor.document.as_ref().unwrap().as_bytes().to_vec();
        attach(editor);

        let partial: ReadLibrary = |_, _, progress| {
            progress.store(19, Ordering::Relaxed);
            Err("Missing preset reply after 19 slots.".into())
        };
        editor.request_refresh(partial);
        editor.confirm_refresh(true, partial);
        wait_for_completion(editor, &ctx);
        assert_eq!(
            editor.error.as_deref(),
            Some("Missing preset reply after 19 slots.")
        );
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), prior);

        let finishes_after_stop: ReadLibrary = |attachment, stop, progress| {
            while !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            complete_capture(attachment, stop, progress)
        };
        for disconnect in [false, true] {
            editor.request_refresh(finishes_after_stop);
            editor.confirm_refresh(true, finishes_after_stop);
            if disconnect {
                editor.observe_device(None);
            } else {
                editor.cancel_operation();
            }
            wait_for_completion(editor, &ctx);
            let document = editor.document.as_ref().unwrap();
            assert_eq!(document.as_bytes(), prior);
            assert!(document.is_dirty() && document.can_undo());
        }
        assert!(editor.refresh.attachment.is_none());
        attach(editor);
        assert!(editor.refresh.attachment.is_some());
    }
}

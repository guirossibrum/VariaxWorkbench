use super::{EditorDocument, FamilyTab, VariaxEditor, egui, live_attachment::Attachment};
use crate::editor::tabs::DeviceTab;
use line6_devices::family::variax::{self, Restore};
use std::{
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

struct Worker {
    stop: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    thread: JoinHandle<Result<Capture, String>>,
}

#[derive(Default)]
pub(super) struct Refresh {
    attachment: Option<Attachment>,
    confirming: bool,
    worker: Option<Worker>,
}

impl Refresh {
    pub(super) fn observe(&mut self, device: Option<&DeviceTab>) {
        let attachment = device.and_then(Attachment::verified);
        if self.attachment != attachment {
            self.cancel();
            self.confirming = false;
            self.attachment = attachment;
        }
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

    fn start(&mut self, read: ReadLibrary) -> Result<(), String> {
        let attachment = self
            .attachment
            .clone()
            .ok_or("The verified Variax attachment is no longer available.")?;
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let worker_stop = stop.clone();
        let worker_progress = progress.clone();
        let thread = std::thread::Builder::new()
            .name("variax-refresh".into())
            .spawn(move || read(&attachment, &worker_stop, &worker_progress))
            .map_err(|error| format!("Could not start Variax Refresh: {error}"))?;
        self.worker = Some(Worker {
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
    pub(super) fn download_button(&mut self, ui: &mut egui::Ui) -> bool {
        if self.refresh.attachment.is_none() || self.follow.disconnected() {
            return false;
        }
        let button = ui.add_enabled(
            cfg!(target_os = "linux") && !self.refresh.busy() && !self.follow.busy(),
            egui::Button::new("Download from guitar…")
                .min_size(egui::vec2(0.0, super::theme::TARGET)),
        );
        if button.clicked() {
            self.request_refresh(read_library);
        }
        true
    }

    fn request_refresh(&mut self, read: ReadLibrary) {
        if self.is_reading()
            || self.follow.busy()
            || self.follow.disconnected()
            || self.blocking_prompt()
            || self.refresh.attachment.is_none()
        {
            return;
        }
        if self.dirty() {
            self.refresh.confirming = true;
        } else {
            self.start_refresh(read);
        }
    }

    fn start_refresh(&mut self, read: ReadLibrary) {
        match self.refresh.start(read) {
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
            ui.label("Download reads the stored presets and bank names. It replaces this document only after the full read succeeds.");
            ui.horizontal(|ui| {
                let cancel = ui.button("Keep editing");
                let replace = ui.button("Download and replace");
                (cancel.clicked(), replace.clicked())
            }).inner
        });
        let (cancel, replace) = dialog.inner;
        if cancel || replace || dialog.should_close() {
            self.confirm_refresh(replace, read_library);
        }
    }

    fn confirm_refresh(&mut self, replace: bool, read: ReadLibrary) {
        if std::mem::take(&mut self.refresh.confirming) && replace {
            self.start_refresh(read);
        }
    }

    pub(super) fn poll_refresh(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.refresh.worker else {
            return;
        };
        let stopping = worker.stop.load(Ordering::Relaxed);
        if !worker.thread.is_finished() {
            if stopping {
                self.status = "Stopping guitar refresh and releasing the bridge…".into();
            } else {
                let completed = worker.progress.load(Ordering::Relaxed);
                let total = variax::Preset::all().count();
                self.status = format!("Reading guitar library · {completed}/{total} presets");
            }
            ctx.request_repaint_after(Duration::from_millis(80));
            return;
        }
        let worker = self.refresh.worker.take().expect("finished worker");
        let result = worker
            .thread
            .join()
            .unwrap_or_else(|_| Err("The Variax Refresh worker stopped unexpectedly.".into()));
        if stopping {
            self.status = "Guitar refresh stopped. The open document was kept.".into();
            self.error = result.err();
            return;
        }
        match result.and_then(|capture| {
            let mut document = EditorDocument::from_unsaved_bundle(capture.bundle)?;
            if let Restore::Switched { to, .. } = capture.restore {
                document.select(usize::from(to.bank()), usize::from(to.slot()))?;
            }
            Ok((document, capture.restore, capture.release_error))
        }) {
            Ok((document, restore, release_error)) => {
                let time = SystemTime::now();
                let snapshot_error = self.preferences_path.as_deref().and_then(|preferences| {
                    super::startup_library::save_download(preferences, &document, time)
                        .err()
                        .map(|error| {
                            format!("Could not remember the downloaded Variax library: {error}")
                        })
                });
                self.adopt_document(document, None);
                self.library_origin = Some(super::LibraryOrigin::Download(time));
                self.status =
                    "Read 60 guitar presets and 12 bank names. Use Save As to keep this library."
                        .into();
                self.error = match restore {
                    Restore::Switched { verified: true, .. } => release_error,
                    other => Some(with_release_error(restore_notice(&other), release_error)),
                };
                if let Some(error) = snapshot_error {
                    self.error = Some(with_release_error(error, self.error.take()));
                }
            }
            Err(error) => {
                self.error = Some(error);
                self.status = "Guitar refresh did not complete. The open document was kept.".into();
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

    pub(crate) fn verified_tab() -> DeviceTab {
        let mut metadata = SyntheticSysfs::new();
        metadata.add_device(
            "8-2.1",
            PODXT_LIVE_USB.0,
            PODXT_LIVE_USB.1,
            "PODxt Live",
            &[],
        );
        let mut detection = detection::DetectionService::new(Box::new(VerifiedRig(metadata)));
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

    #[test]
    fn verified_refresh_confirms_dirty_replacement_and_loads_an_unsaved_bundle() {
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
        assert!(document.is_dirty());
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

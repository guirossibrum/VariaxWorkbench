//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use variax_workbench::device_probe::{
    self, Consent, HostRig, LinuxSysfs, ProbeMode, ProbeSession, RigBackend, Stage, StageStatus,
};
use variax_workbench::firmware_notice::{Device as FirmwareDevice, FirmwareNotice};
use variax_workbench::hardware_gates;
#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
use variax_workbench::hardware_gates::Gate;
use variax_workbench::pod_preset;
use variax_workbench::recording;
use variax_workbench::rig::{ProductProfile, Rejection, RoleKind, Session};
use variax_workbench::{document::EditorDocument, preset::PresetFile};

#[path = "families/variax/editor/mod.rs"]
mod variax_panel;
use variax_workbench::ui::{connections::Connections, theme};

#[path = "families/hd500x/editor/mod.rs"]
mod hd500x_panel;
#[path = "families/podxt/editor/mod.rs"]
mod podxt_panel;

#[path = "tools/recording/panel.rs"]
mod recording_panel;

#[path = "shared/picker.rs"]
mod picker;
use picker::{FileFilter, PickFuture, PickRequest, Picker};

#[path = "app/detection/mod.rs"]
mod detection;
use detection::{DetectionService, Scan};

#[path = "app/registry.rs"]
mod registry;
#[path = "app/shell/mod.rs"]
mod shell;
#[path = "app/shell/tabs.rs"]
mod tabs;

use tabs::{
    CloseResult, EditorDefaults, EditorServices, FamilyInfo, FamilyTab, ProductFamily, TabEntry,
    TabId, TabSource, Tabs,
};

#[path = "app/shell/window.rs"]
mod window;
use window::WindowPreferences;

use shell::{CloseRequest, DetectConfirmation};

trait FileDialogs {
    fn start_picker(&mut self, request: PickRequest) -> PickFuture;

    #[cfg(test)]
    fn open_wav(&mut self) -> Option<PathBuf>;
    #[cfg(test)]
    fn save_wav(&mut self, suggested: &str) -> Option<PathBuf>;
    #[cfg(test)]
    fn save_aiff(&mut self, suggested: &str) -> Option<PathBuf>;
    fn open_pod_tone(&mut self) -> Option<PathBuf>;
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf>;
    #[cfg(test)]
    fn pick_folder(&mut self) -> Option<PathBuf>;
    #[cfg(test)]
    fn queue_pod_save(&mut self, _path: Option<PathBuf>) {
        panic!("POD save queue is available only on fake dialogs");
    }
}

#[cfg(not(test))]
struct NativeDialogs;
#[cfg(not(test))]
impl FileDialogs for NativeDialogs {
    fn start_picker(&mut self, request: PickRequest) -> PickFuture {
        request.native()
    }

    fn open_pod_tone(&mut self) -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        return variax_workbench::macos_dialog::Dialog::Open.select();
        #[cfg(not(target_os = "macos"))]
        rfd::FileDialog::new().pick_file()
    }
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        return variax_workbench::macos_dialog::Dialog::Save(suggested.to_owned()).select();
        #[cfg(not(target_os = "macos"))]
        rfd::FileDialog::new().set_file_name(suggested).save_file()
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeDialogs {
    next_picker: Option<PickFuture>,
    open: std::collections::VecDeque<Option<PathBuf>>,
    save: std::collections::VecDeque<Option<PathBuf>>,
    artwork: std::collections::VecDeque<Option<PathBuf>>,
    wav: std::collections::VecDeque<Option<PathBuf>>,
    pod_open: std::collections::VecDeque<Option<PathBuf>>,
    pod_save: std::collections::VecDeque<Option<PathBuf>>,
    #[cfg_attr(test, allow(dead_code))]
    folder: std::collections::VecDeque<Option<PathBuf>>,
}
#[cfg(test)]
impl FileDialogs for FakeDialogs {
    fn start_picker(&mut self, request: PickRequest) -> PickFuture {
        if let Some(future) = self.next_picker.take() {
            return future;
        }
        let path = match request {
            PickRequest::OpenEditorFile(filters) => {
                debug_assert!(
                    filters
                        .iter()
                        .all(|filter| !filter.name.is_empty() && !filter.extensions.is_empty())
                );
                self.open.pop_front().expect("unexpected open picker")
            }
            PickRequest::OpenWorkbenchFile => {
                self.open.pop_front().expect("unexpected open picker")
            }
            PickRequest::SaveWorkbenchFile { name, extension } => {
                debug_assert!(!name.is_empty() && !extension.is_empty());
                self.save.pop_front().expect("unexpected save picker")
            }
            PickRequest::Artwork => self.artwork.pop_front().expect("unexpected artwork picker"),
            PickRequest::OpenWav => self.open_wav(),
            PickRequest::SaveAudio { name, aiff: true } => self.save_aiff(&name),
            PickRequest::SaveAudio { name, aiff: false } => self.save_wav(&name),
            PickRequest::OpenTone => self.open_pod_tone(),
            PickRequest::SaveTone { name } => self.save_pod_tone(&name),
            PickRequest::Folder => self.pick_folder(),
        };
        Box::pin(std::future::ready(path))
    }

    fn open_wav(&mut self) -> Option<PathBuf> {
        self.wav.pop_front().expect("unexpected wav dialog")
    }
    fn save_wav(&mut self, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected audio save dialog")
    }
    fn save_aiff(&mut self, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected AIFF save dialog")
    }
    fn open_pod_tone(&mut self) -> Option<PathBuf> {
        self.pod_open
            .pop_front()
            .expect("unexpected POD tone dialog")
    }
    fn save_pod_tone(&mut self, _: &str) -> Option<PathBuf> {
        self.pod_save
            .pop_front()
            .expect("unexpected POD save dialog")
    }
    fn pick_folder(&mut self) -> Option<PathBuf> {
        self.folder.pop_front().expect("unexpected folder dialog")
    }
    fn queue_pod_save(&mut self, path: Option<PathBuf>) {
        self.pod_save.push_back(path);
    }
}

type TabPickerResult =
    Box<dyn FnOnce(&mut dyn FamilyTab, Option<PathBuf>, &mut EditorServices<'_>)>;

enum PickerAction {
    OpenFile,
    Backup(BackupChoice, Option<detection::DeviceKey>),
    SaveReport {
        restart: bool,
    },
    LoadRecording,
    SaveRecording {
        aiff: bool,
        quit: bool,
    },
    #[allow(dead_code)] // Editor-lane migration, see docs/non-blocking-pickers.md.
    Tab {
        id: TabId,
        complete: TabPickerResult,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackupChoice {
    Variax,
    VariaxFull,
    Podxt,
}

#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(not(feature = "usb"), allow(dead_code))]
enum DeviceOperationEvent<T> {
    Progress(String),
    Finished(Result<T, String>),
}

fn backup_offered(choice: BackupChoice) -> bool {
    #[cfg(feature = "usb")]
    {
        use variax_workbench::library_backup::{Which, offered_for};
        offered_for(match choice {
            BackupChoice::Podxt => Which::Podxt,
            BackupChoice::Variax | BackupChoice::VariaxFull => Which::Variax,
        })
    }
    #[cfg(not(feature = "usb"))]
    {
        let _ = choice;
        false
    }
}

fn tone_offered() -> bool {
    #[cfg(feature = "usb")]
    {
        variax_workbench::podxt_live::offered()
    }
    #[cfg(not(feature = "usb"))]
    {
        false
    }
}

fn device_check_platform_notice() -> Option<&'static str> {
    if cfg!(all(test, target_os = "macos")) {
        Some(variax_workbench::MAC_CONNECTION_UNAVAILABLE)
    } else {
        variax_workbench::offline_device_notice()
    }
}

fn parse_tone_channel(text: &str) -> Result<String, String> {
    #[cfg(feature = "usb")]
    {
        variax_workbench::podxt_live::parse_channel(text).map(|location| location.label())
    }
    #[cfg(not(feature = "usb"))]
    {
        let _ = text;
        Err("this build does not open a device for a tone check; nothing was sent".into())
    }
}

fn channel_does(label: &str) -> String {
    format!(
        "Switches the PODxt Live to channel {label}, like the footswitch, and leaves it there. \
         The app reads the saved tones first. If the current tone has unsaved edits, nothing is \
         switched. Nothing is saved. This can take a few minutes. Leave this window open until \
         it finishes."
    )
}

const BACKUP_INTRO: &str = "Backup is separate from the steps above. Step 4 stays off. Nothing is written back to either device. No is the default. You choose a folder, and a new folder is made inside it.";
const BACKUP_VARIAX: &str = "Back up the Variax";
const BACKUP_PODXT: &str = "Back up the PODxt Live";
const BACKUP_VARIAX_DOES: &str = "Reads all 60 guitar presets. The guitar changes preset while this runs, then the app tries to put the original preset back. Leave this window open until it finishes.";
const BACKUP_PODXT_DOES: &str = "Reads the stored tones and setups. The current tone is not changed on purpose. It also asks which model packs are installed. Leave this window open until it finishes.";
const BACKUP_VARIAX_FULL: &str = "Back up all Variax program bytes";
const BACKUP_VARIAX_FULL_DOES: &str = "Reads 60 presets and 35 data chunks per preset (2,100 reads). Allow several minutes. The guitar changes presets while the backup runs. The app then tries to reselect the original. Leave the guitar controls alone.";
const BACKUP_VARIAX_FULL_YES: &str = "Yes, back up all program bytes";
const BACKUP_VARIAX_YES: &str = "Yes, back up the Variax";
const BACKUP_PODXT_YES: &str = "Yes, back up the PODxt Live";
const TONE_INTRO: &str = "Tone checks are separate from the steps above. Step 4 stays off. No is the default. Nothing is saved onto the PODxt.";
const TONE_TRY: &str = "Try a change and put it back";
const TONE_TRY_DOES: &str = "Changes treble, mid, one EQ band, and the modulation, delay and reverb switches, and moves modulation before or after the amp. It reads them back, then puts the original values back. The stomp is not changed. Nothing is saved. Leave this window open until it finishes.";
const TONE_TRY_YES: &str = "Yes, try it and put it back";
const TONE_CHANNEL: &str = "Play a saved tone";
const TONE_CHANNEL_YES: &str = "Yes, play that saved tone";
const TONE_KEEP: &str = "Leaving a change on the PODxt stays off.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToneChoice {
    Try,
    Channel,
}

struct Workbench {
    dialogs: Box<dyn FileDialogs>,
    picker: Picker<PickerAction>,
    help: bool,
    permit_close: bool,
    theme_polled: Instant,
    theme_text: String,
    window_preferences: Option<WindowPreferences>,
    screenshot: Option<PathBuf>,
    frame_count: u32,
    recording_open: bool,
    recorder: recording::Panel,
    device_check_open: bool,
    device_check_notice: String,
    /// Backup question waiting for Yes or No. Nothing is sent until Yes.
    backup_choice: Option<BackupChoice>,
    backup_rx: Option<
        std::sync::mpsc::Receiver<
            DeviceOperationEvent<variax_workbench::variax_library::BackupResult>,
        >,
    >,
    backup_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Tone question waiting for Yes or No. Nothing is sent until Yes.
    tone_choice: Option<ToneChoice>,
    tone_channel: String,
    tone_rx: Option<std::sync::mpsc::Receiver<DeviceOperationEvent<String>>>,
    /// True only after the step 2 report from this session was saved. A later
    /// answer of step 2 clears it. Restarting for the G1 check requires it.
    device_check_step_two_saved: bool,
    device_check: ProbeSession<RigBackend>,
    device_check_focus: Option<Stage>,
    #[cfg(test)]
    pending_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    recording_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    recording_visible: String,
    #[cfg(test)]
    device_check_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    device_check_button_ids: HashMap<&'static str, egui::Id>,
    #[cfg(test)]
    device_check_visible: String,
    #[cfg(test)]
    recording_capture_attempts: u32,
    tabs: Tabs,
    connections: Connections,
    startup_connection: bool,
    connection_inventory: shell::ConnectionInventory,
    detection: Option<DetectionService>,
    detect_confirmation: Option<DetectConfirmation>,
    inventory_refresh: detection::InventoryRefresh,
    next_detection: Instant,
    detection_status: Option<String>,
    #[cfg(test)]
    detection_buttons: HashMap<&'static str, Rect>,
    defaults: EditorDefaults,
    shell_error: Option<String>,
    shell_status: String,
    close_request: Option<CloseRequest>,
    waiting_for_hardware: bool,
    closing_tab: Option<TabId>,
    recording_close_pending: bool,
    #[cfg(test)]
    tab_buttons: HashMap<&'static str, Rect>,
    /// When set, the header uses the macOS traffic-light inset. Production
    /// macOS always does; Linux tests turn this on to measure that inset.
    #[cfg(test)]
    clear_window_controls: bool,
}

impl Workbench {
    fn refresh_theme(&mut self, ctx: &egui::Context) {
        if self.theme_polled.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.theme_polled = Instant::now();
        let text = std::env::var_os("HOME")
            .and_then(|home| {
                fs::read_to_string(
                    Path::new(&home).join(".local/state/omarchy/current/theme/colors.toml"),
                )
                .ok()
            })
            .unwrap_or_default();
        if text != self.theme_text {
            self.theme_text = text;
            let color = |key: &str, default| {
                self.theme_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once('=')?;
                        if name.trim() != key {
                            return None;
                        }
                        let value = value.trim().trim_matches('"').strip_prefix('#')?;
                        let value = u32::from_str_radix(value, 16).ok()?;
                        Some(Color32::from_rgb(
                            (value >> 16) as u8,
                            (value >> 8) as u8,
                            value as u8,
                        ))
                    })
                    .unwrap_or(default)
            };
            let accent = color("accent", Color32::from_rgb(255, 214, 10));
            let background = color("background", Color32::from_rgb(26, 27, 38));
            ctx.style_mut(|style| {
                style.visuals.selection.bg_fill = accent.gamma_multiply(0.35);
                style.visuals.hyperlink_color = accent;
                style.visuals.panel_fill = background;
            });
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

impl eframe::App for Workbench {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(window) = &mut self.window_preferences {
            ctx.input(|input| window.observe(input.viewport(), input.viewport_rect().size()));
        }
        self.render(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.permit_close
            && let Some(window) = &self.window_preferences
            && let Err(error) = window.save()
        {
            eprintln!("Could not save window preferences: {error}");
        }
    }
}

impl Workbench {
    fn device_check_button(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: &str,
    ) -> egui::Response {
        let response = ui.add(
            egui::Button::new(egui::RichText::new(label).size(theme::BODY))
                .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
        );
        #[cfg(test)]
        {
            self.device_check_buttons.insert(id, response.rect);
            self.device_check_button_ids.insert(id, response.id);
        }
        #[cfg(not(test))]
        let _ = id;
        response
    }

    fn new_device_check() -> ProbeSession<RigBackend> {
        #[cfg(not(test))]
        let reader = LinuxSysfs::new();
        #[cfg(test)]
        let reader = {
            // The default headless window uses a private empty fixture. Tests
            // needing a reply inject either a fake tree or a scripted rig.
            static FIXTURE: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
            LinuxSysfs::with_root(FIXTURE.get_or_init(|| tempfile::tempdir().unwrap()).path())
        };
        let rig = HostRig::new(reader);
        #[cfg(all(target_os = "linux", feature = "usb", not(test)))]
        let rig = rig.with_recognition_lock();
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        let rig = rig.with_macos_usb();
        ProbeSession::new(RigBackend::ThisComputer(rig))
    }

    fn restart_device_check(&mut self) {
        #[cfg(not(test))]
        let probe = Self::new_device_check();
        #[cfg(test)]
        let probe = ProbeSession::new(match self.device_check.backend() {
            RigBackend::ThisComputer(rig) => {
                RigBackend::ThisComputer(HostRig::new(rig.reader().clone()))
            }
            RigBackend::Simulated(rig) => {
                RigBackend::Simulated(device_probe::SimulatedRig::new(rig.scenario()))
            }
        });
        self.device_check = probe;
        self.device_check_focus = None;
        self.device_check_step_two_saved = false;
        self.device_check_notice.clear();
    }

    /// Saves the sanitized report only. A failure is shown as a fixed phrase
    /// and does not answer or rerun a stage.
    fn note_sanitized_report_save(&mut self, path: &Path) {
        self.device_check_notice = match self.device_check.save_sanitized_report(path) {
            Ok(()) => {
                if self.device_check.identification_answered() {
                    self.device_check_step_two_saved = true;
                }
                "Steps 1–2 report saved.".into()
            }
            Err(error) => error.to_string(),
        };
    }

    fn poll_backup(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.backup_rx.as_ref() else {
            return;
        };
        let mut finished = None;
        loop {
            match rx.try_recv() {
                Ok(DeviceOperationEvent::Progress(line)) => self.device_check_notice = line,
                Ok(DeviceOperationEvent::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    finished = Some(Err("The device operation ended without a result.".into()));
                    break;
                }
            }
        }
        if let Some(result) = finished {
            self.backup_rx = None;
            self.backup_cancel = None;
            use variax_workbench::variax_library::BackupResult;
            self.device_check_notice = match result {
                Ok(BackupResult::Variax { notice, .. } | BackupResult::Podxt { notice, .. })
                | Err(notice) => notice,
            };
            self.shell_status = "Backup finished. See Device check for the result.".into();
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn firmware_notices(&self) -> impl Iterator<Item = FirmwareNotice> + '_ {
        self.detection
            .iter()
            .flat_map(DetectionService::firmware_notices)
            .chain(self.device_check.firmware_notice())
    }

    fn firmware_unverified(&self, device: FirmwareDevice) -> bool {
        self.firmware_notices()
            .any(|notice| notice.device() == device)
    }

    fn backup_firmware_blocked(&self, choice: BackupChoice) -> bool {
        self.firmware_unverified(FirmwareDevice::Podxt)
            || (choice != BackupChoice::Podxt && self.firmware_unverified(FirmwareDevice::Variax))
    }

    fn verified_backup_guitar(&self) -> Option<&detection::DeviceKey> {
        let mut guitars = self.tabs.entries.iter().filter_map(|entry| {
            let tab = entry.device.as_ref()?;
            (tab.device.key.family_id == "variax" && tab.verified_attachment().is_some())
                .then_some(&tab.device.key)
        });
        match (guitars.next(), guitars.next()) {
            (Some(key), None) => Some(key),
            _ => None,
        }
    }

    fn start_backup(&mut self, ctx: &egui::Context, choice: BackupChoice) {
        if self.backup_firmware_blocked(choice) {
            return;
        }
        let target = if choice == BackupChoice::VariaxFull {
            let Some(guitar) = self.verified_backup_guitar() else {
                self.device_check_notice = "Wait for a verified Variax connection.".into();
                return;
            };
            Some(guitar.clone())
        } else {
            None
        };
        if self.request_picker(
            ctx,
            PickRequest::Folder,
            PickerAction::Backup(choice, target),
        ) {
            self.backup_choice = None;
        }
    }

    fn start_backup_in(
        &mut self,
        choice: BackupChoice,
        target: Option<detection::DeviceKey>,
        root: Option<PathBuf>,
    ) {
        if self.backup_firmware_blocked(choice) {
            return;
        }
        let Some(root) = root else {
            self.device_check_notice = "No folder chosen. Nothing was sent.".into();
            return;
        };
        if choice == BackupChoice::VariaxFull
            && (target.is_none() || self.verified_backup_guitar() != target.as_ref())
        {
            self.device_check_notice =
                "The verified Variax connection changed. Nothing was sent.".into();
            return;
        }
        #[cfg(all(not(test), feature = "usb"))]
        {
            use variax_workbench::library_backup::{self, Which};
            use variax_workbench::variax_library::{BackupResult, LibraryPreferences};

            let (tx, rx) = std::sync::mpsc::channel();
            self.backup_rx = Some(rx);
            let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            if choice == BackupChoice::VariaxFull {
                self.backup_cancel = Some(cancelled.clone());
            }
            self.device_check_notice = "Backup started. Leave this window open.".into();
            let preferences = self.defaults.preferences.clone();
            std::thread::spawn(move || {
                let progress = |line: &str| {
                    let _ = tx.send(DeviceOperationEvent::Progress(line.to_owned()));
                };
                let mut result = match choice {
                    BackupChoice::Variax => library_backup::run(Which::Variax, &root, progress),
                    BackupChoice::Podxt => library_backup::run(Which::Podxt, &root, progress),
                    BackupChoice::VariaxFull => {
                        let observed = target
                            .as_ref()
                            .and_then(|key| key.attachment.usb_location());
                        observed
                            .ok_or_else(|| "No verified Variax attachment.".to_owned())
                            .and_then(|location| {
                                library_backup::run_full_variax(
                                    &root, location, &cancelled, progress,
                                )
                            })
                    }
                };
                if let Some(path) = preferences.as_deref()
                    && let Ok(backup_result) = &mut result
                {
                    let preferences = LibraryPreferences::new(path);
                    let (saved, notice) = match backup_result {
                        BackupResult::Variax { backup, notice } => {
                            (preferences.remember_backup(backup), notice)
                        }
                        BackupResult::Podxt { backup, notice } => {
                            (preferences.remember_podxt_backup(backup), notice)
                        }
                    };
                    if let Err(error) = saved {
                        notice.push_str(&format!(
                            " Could not remember this library for startup: {error}"
                        ));
                    }
                }
                let _ = tx.send(DeviceOperationEvent::Finished(result));
            });
        }
        #[cfg(any(test, not(feature = "usb")))]
        {
            let _ = root;
            self.device_check_notice =
                "this build does not open a device for a backup; nothing was sent".into();
        }
    }

    fn cancel_full_backup(&self) {
        if let Some(cancel) = &self.backup_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn poll_tone(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.tone_rx.as_ref() else {
            return;
        };
        let mut finished = None;
        loop {
            match rx.try_recv() {
                Ok(DeviceOperationEvent::Progress(line)) => self.device_check_notice = line,
                Ok(DeviceOperationEvent::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    finished = Some(Err("The device operation ended without a result.".into()));
                    break;
                }
            }
        }
        if let Some(result) = finished {
            self.tone_rx = None;
            self.device_check_notice = match result {
                Ok(text) => text,
                Err(text) => text,
            };
            self.shell_status = "Tone check finished. See Device check for the result.".into();
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn start_tone(&mut self, choice: ToneChoice) {
        self.tone_choice = None;
        if self.firmware_unverified(FirmwareDevice::Podxt) {
            return;
        }
        if self.backup_rx.is_some() || self.tone_rx.is_some() {
            self.device_check_notice = "Wait until the current check finishes.".into();
            return;
        }
        #[cfg(test)]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a tone check; nothing was sent".into();
        }
        #[cfg(all(not(test), feature = "usb"))]
        {
            let channel = self.tone_channel.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.tone_rx = Some(rx);
            self.device_check_notice = "Tone check started. Leave this window open.".into();
            std::thread::spawn(move || {
                let tx_progress = tx.clone();
                let result = match choice {
                    ToneChoice::Try => variax_workbench::podxt_live::run_try(|line| {
                        let _ = tx_progress.send(DeviceOperationEvent::Progress(line.to_owned()));
                    }),
                    ToneChoice::Channel => {
                        variax_workbench::podxt_live::run_channel(&channel, |line| {
                            let _ =
                                tx_progress.send(DeviceOperationEvent::Progress(line.to_owned()));
                        })
                    }
                };
                let _ = tx.send(DeviceOperationEvent::Finished(result));
            });
        }
        #[cfg(all(not(test), not(feature = "usb")))]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a tone check; nothing was sent".into();
        }
    }

    fn device_check_window(&mut self, ctx: &egui::Context) {
        self.poll_backup(ctx);
        self.poll_tone(ctx);
        if !self.device_check_open {
            return;
        }
        #[cfg(test)]
        {
            self.device_check_buttons.retain(|id, _| *id == "open");
            self.device_check_button_ids.clear();
        }
        let pod_unverified = self.firmware_unverified(FirmwareDevice::Podxt);
        let variax_backup_blocked = self.backup_firmware_blocked(BackupChoice::Variax);
        let mut notices: Vec<_> = self.firmware_notices().collect();
        let staged_notice = self.device_check.firmware_notice();
        notices.retain(|notice| Some(*notice) != staged_notice);
        if self
            .backup_choice
            .is_some_and(|choice| self.backup_firmware_blocked(choice))
        {
            self.backup_choice = None;
        }
        if pod_unverified {
            self.tone_choice = None;
        }
        let mut open = true;
        let mut answer = None;
        let mut restart = false;
        let mut verify_g1 = false;
        let mut save_report = false;
        let mut backup = None;
        let timed_out = self.device_check.identification_timed_out();
        let mut draw = |ui: &mut egui::Ui, app: &mut Self| {
            if app.has_prompt() || app.picker.is_open() {
                ui.disable();
            }
            if app.device_check.mode() == ProbeMode::ThisComputer
                && let Some(notice) = device_check_platform_notice()
            {
                ui.label(notice);
                #[cfg(test)]
                {
                    app.device_check_visible = notice.into();
                }
                return;
            }
            let mut visible = String::new();
            let banner = device_probe::availability_banner();
            visible.push_str(&banner);
            let mut gate_notes = vec![banner];
            let label = device_probe::this_computer_label();
            ui.label(label);
            visible.push_str(&format!("\n{label}"));
            ui.weak(
                "Each step asks first, the answer is No unless you choose Yes, and a passed \
                     step never starts the next one.",
            );
            ui.label(device_probe::step_two_preflight());
            ui.label(device_probe::CANCEL_LIMIT);
            visible.push_str(&format!(
                "\n{}\n{}",
                device_probe::step_two_preflight(),
                device_probe::CANCEL_LIMIT
            ));
            for notice in &notices {
                let text = notice.message();
                ui.colored_label(theme::AMBER, &text);
                visible.push_str(&format!("\n{text}"));
            }
            if !timed_out {
                restart = app
                    .device_check_button(ui, "restart", "Start over")
                    .clicked();
            }
            ui.separator();
            for stage in Stage::LADDER {
                let status = app.device_check.status(stage);
                let waiting = app.device_check.awaiting() == Some(stage);
                let unavailable = app.device_check.locked(stage);
                let gate_locked = hardware_gates::is_locked(stage.gate());
                let (mark, colour, state) = match status {
                    StageStatus::Passed => ("✔", theme::OK, status.label()),
                    StageStatus::Stopped => ("■", theme::DANGER, status.label()),
                    StageStatus::NotRun if unavailable && gate_locked => {
                        ("○", theme::DIM, "locked in this build")
                    }
                    StageStatus::NotRun if unavailable => {
                        ("○", theme::DIM, "unavailable in this build")
                    }
                    StageStatus::NotRun if waiting => {
                        ("▶", theme::AMBER, "waiting for your answer")
                    }
                    StageStatus::NotRun => ("○", theme::DIM, status.label()),
                };
                let lock = if device_probe::live_stage_available(stage) {
                    "real device: open"
                } else if gate_locked {
                    "real device: locked"
                } else {
                    "real device: unavailable"
                };
                visible.push_str(&format!("\nstep {} {state} {lock}", stage.number()));
                gate_notes.push(format!(
                    "Step {} · {} ({}) · {lock}",
                    stage.number(),
                    stage.title(),
                    stage.gate_label()
                ));
                ui.horizontal(|ui| {
                    ui.colored_label(colour, mark);
                    ui.label(
                        egui::RichText::new(format!("Step {} · {}", stage.number(), stage.title()))
                            .size(theme::BODY)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(egui::RichText::new(state).size(theme::STATUS).color(colour));
                });
                if !(timed_out && stage == Stage::Identification)
                    && let Some(detail) = app.device_check.detail(stage)
                {
                    ui.indent(("device-check-detail", stage.number()), |ui| {
                        ui.label(detail);
                    });
                    visible.push_str(&format!(": {detail}"));
                }
            }
            ui.collapsing("Details", |ui| {
                for note in &gate_notes {
                    ui.label(note);
                }
            });
            ui.separator();
            if let Some(stage) = app.device_check.awaiting() {
                ui.heading(format!("Step {}: {}", stage.number(), stage.title()));
                ui.strong("What it does");
                ui.label(stage.action());
                ui.strong("Risk");
                ui.label(stage.risk());
                visible.push_str(&format!("\n{}\n{}", stage.action(), stage.risk()));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let no = app.device_check_button(ui, "no", "No, stop");
                    if ui.is_enabled()
                        && (app.device_check_focus != Some(stage)
                            || ui.memory(|memory| memory.focused().is_none()))
                    {
                        no.request_focus();
                        if !ui.clip_rect().contains_rect(no.rect) {
                            no.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
                        }
                        app.device_check_focus = Some(stage);
                    }
                    if no.clicked() {
                        answer = Some(Consent::No);
                    }
                    let yes = device_probe::consent_yes_label(ProbeMode::ThisComputer, stage);
                    visible.push_str(yes);
                    let enabled = stage != Stage::SettingsRead || !variax_backup_blocked;
                    ui.add_enabled_ui(enabled, |ui| {
                        if app.device_check_button(ui, "yes", yes).clicked() {
                            answer = Some(Consent::Yes);
                        }
                    });
                });
            } else if !timed_out
                && app.device_check.status(Stage::UsbRecognition) != StageStatus::NotRun
            {
                let outcome = match app.device_check.stopped() {
                    Some((stage, _)) => format!(
                        "Stopped at step {}. Nothing further runs; choose Start over to try again.",
                        stage.number()
                    ),
                    None => device_probe::this_computer_finished_message().into(),
                };
                ui.label(&outcome);
                visible.push_str(&format!("\n{outcome}"));
            }
            if backup_offered(BackupChoice::Podxt) || backup_offered(BackupChoice::Variax) {
                ui.separator();
                ui.heading("Backup");
                ui.label(BACKUP_INTRO);
                visible.push_str(BACKUP_INTRO);
                if app.backup_rx.is_some() {
                    ui.label("Backup is running. Leave this window open.");
                    visible.push_str("\nBackup is running.");
                    if let Some(cancel) = &app.backup_cancel {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            ui.label(
                                "Stopping after the current request and returning the driver.",
                            );
                        } else if app
                            .device_check_button(ui, "backup-stop", "Stop full backup")
                            .clicked()
                        {
                            app.cancel_full_backup();
                        }
                    }
                } else if let Some(choice) = app.backup_choice {
                    let (does, yes) = match choice {
                        BackupChoice::Variax => (BACKUP_VARIAX_DOES, BACKUP_VARIAX_YES),
                        BackupChoice::VariaxFull => {
                            (BACKUP_VARIAX_FULL_DOES, BACKUP_VARIAX_FULL_YES)
                        }
                        BackupChoice::Podxt => (BACKUP_PODXT_DOES, BACKUP_PODXT_YES),
                    };
                    ui.label(does);
                    visible.push_str(does);
                    ui.horizontal(|ui| {
                        let no = app.device_check_button(ui, "backup-no", "No, stop");
                        if no.clicked() {
                            app.backup_choice = None;
                            app.device_check_notice = "Nothing was read.".into();
                        }
                        if app.device_check_button(ui, "backup-yes", yes).clicked() {
                            backup = Some(choice);
                        }
                    });
                    visible.push_str(yes);
                } else {
                    ui.horizontal(|ui| {
                        if backup_offered(BackupChoice::Variax) && !variax_backup_blocked {
                            visible.push_str(BACKUP_VARIAX);
                            if app
                                .device_check_button(ui, "backup-variax", BACKUP_VARIAX)
                                .clicked()
                            {
                                app.backup_choice = Some(BackupChoice::Variax);
                            }
                        }
                        if backup_offered(BackupChoice::Podxt) && !pod_unverified {
                            visible.push_str(BACKUP_PODXT);
                            if app
                                .device_check_button(ui, "backup-podxt", BACKUP_PODXT)
                                .clicked()
                            {
                                app.backup_choice = Some(BackupChoice::Podxt);
                            }
                        }
                    });
                    if backup_offered(BackupChoice::VariaxFull)
                        && !variax_backup_blocked
                        && app.verified_backup_guitar().is_some()
                    {
                        if app
                            .device_check_button(ui, "backup-variax-full", BACKUP_VARIAX_FULL)
                            .clicked()
                        {
                            app.backup_choice = Some(BackupChoice::VariaxFull);
                        }
                        visible.push_str(BACKUP_VARIAX_FULL);
                    }
                }
            }
            if tone_offered() && (!pod_unverified || app.tone_rx.is_some()) {
                ui.separator();
                ui.heading("PODxt Live tone");
                ui.label(TONE_INTRO);
                visible.push_str(TONE_INTRO);
                ui.label(TONE_KEEP);
                visible.push_str(TONE_KEEP);
                if app.tone_rx.is_some() {
                    ui.label("The tone check is running. Leave this window open.");
                    visible.push_str("\nThe tone check is running.");
                } else if let Some(choice) = app.tone_choice {
                    let (does, yes) = match choice {
                        ToneChoice::Try => (TONE_TRY_DOES.to_owned(), TONE_TRY_YES),
                        ToneChoice::Channel => {
                            (channel_does(app.tone_channel.trim()), TONE_CHANNEL_YES)
                        }
                    };
                    ui.label(&does);
                    visible.push_str(&does);
                    ui.horizontal(|ui| {
                        let no = app.device_check_button(ui, "tone-no", "No, stop");
                        if no.clicked() {
                            app.tone_choice = None;
                            app.device_check_notice = "Nothing was changed.".into();
                        }
                        if app.device_check_button(ui, "tone-yes", yes).clicked() {
                            app.start_tone(choice);
                        }
                    });
                    visible.push_str(yes);
                } else {
                    ui.horizontal(|ui| {
                        if app.device_check_button(ui, "tone-try", TONE_TRY).clicked() {
                            app.tone_choice = Some(ToneChoice::Try);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Channel");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.tone_channel)
                                .desired_width(80.0)
                                .hint_text("1A"),
                        );
                        if app
                            .device_check_button(ui, "tone-channel", TONE_CHANNEL)
                            .clicked()
                        {
                            match parse_tone_channel(&app.tone_channel) {
                                Ok(label) => {
                                    app.tone_channel = label;
                                    app.tone_choice = Some(ToneChoice::Channel);
                                }
                                Err(error) => app.device_check_notice = error,
                            }
                        }
                    });
                    visible.push_str(TONE_TRY);
                    visible.push_str(TONE_CHANNEL);
                    visible.push_str(&app.tone_channel);
                }
            }
            #[cfg(feature = "usb")]
            if app.backup_rx.is_none() && app.tone_rx.is_none() {
                let mut podxts = app.tabs.entries.iter().filter(|entry| {
                    entry.family.info().id == "podxt"
                        && entry
                            .device
                            .as_ref()
                            .and_then(tabs::DeviceTab::verified_attachment)
                            .is_some()
                });
                if let (Some(id), None) = (podxts.next().map(|entry| entry.id), podxts.next()) {
                    app.with_tab(id, |entry, services| {
                        if let Some(editor) = (&mut *entry.editor as &mut dyn std::any::Any)
                            .downcast_mut::<podxt_panel::PodxtEditor>()
                        {
                            editor.device_check_panel(ui, services);
                        }
                    });
                }
            }
            let report = app.device_check.report();
            let sanitized = app.device_check.sanitized_report();
            ui.horizontal(|ui| {
                if app
                    .device_check_button(ui, "copy", "Copy detailed report (#125)")
                    .clicked()
                {
                    ui.ctx().copy_text(report.clone());
                }
            });
            let reports_note = device_probe::reports_panel_note();
            ui.collapsing("Reports", |ui| {
                ui.label("Detailed report (schema 1), for issue #125.");
                ui.monospace(&report);
                ui.label(reports_note);
                ui.monospace(&sanitized);
            });
            visible.push_str(reports_note);
            if !timed_out {
                save_report = app
                    .device_check_button(ui, "save-report", "Save steps 1–2 report")
                    .clicked();
                if app.device_check.identification_answered() {
                    visible.push_str(device_probe::SAVE_THEN_CHECK_AGAIN);
                    verify_g1 = app
                        .device_check_button(ui, "verify-g1", device_probe::SAVE_THEN_CHECK_AGAIN)
                        .clicked();
                }
                if !app.device_check_notice.is_empty() {
                    ui.label(&app.device_check_notice);
                    visible.push_str(&format!("\n{}", app.device_check_notice));
                }
            }
            #[cfg(test)]
            {
                app.device_check_visible = visible;
            }
            #[cfg(not(test))]
            let _ = visible;
        };
        // Reserve the top-bar offset. egui accounts for the window title
        // within these bounds, then the scroll area can shrink to its content.
        let mut bounds = ctx.content_rect().shrink(8.0);
        bounds.min.y = (bounds.min.y + 48.0).min(bounds.max.y);
        // The title bar grows with the 32px target and the theme margin. Keep the
        // frame margin at egui's 6 so this window still ends inside a short screen.
        let frame = egui::Frame::new()
            .fill(theme::RAISED)
            .stroke(egui::Stroke::new(1.0_f32, theme::LINE))
            .corner_radius(theme::RADIUS_PANEL)
            .inner_margin(egui::Margin::same(6))
            .shadow(ctx.style().visuals.window_shadow);
        let mut timeout_restart = false;
        let mut timeout_save = None;
        egui::Window::new("Device check")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .frame(frame)
            .default_width(640.0)
            .min_height(bounds.height())
            .constrain_to(bounds)
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                if timed_out {
                    ui.add_enabled_ui(!self.has_prompt() && !self.picker.is_open(), |ui| {
                        ui.strong("Step 2 · Variax identity timed out");
                        if let Some(detail) = self.device_check.detail(Stage::Identification) {
                            ui.label(detail);
                        }
                        ui.horizontal_wrapped(|ui| {
                            if self.device_check_step_two_saved {
                                timeout_restart = self
                                    .device_check_button(ui, "retry-g1", "Start over")
                                    .clicked();
                            } else if self
                                .device_check_button(ui, "verify-g1", "Save report and start over")
                                .clicked()
                            {
                                timeout_save = Some(true);
                            }
                            if self
                                .device_check_button(ui, "save-report", "Save report")
                                .clicked()
                            {
                                timeout_save = Some(false);
                            }
                        });
                        if !self.device_check_notice.is_empty() {
                            ui.label(&self.device_check_notice);
                        }
                    });
                    ui.separator();
                }
                egui::ScrollArea::vertical().show(ui, |ui| draw(ui, self));
            });
        restart |= timeout_restart;
        if let Some(retry) = timeout_save {
            verify_g1 = retry;
            save_report = !retry;
        }
        // Close wins over Yes and Start over in this frame.
        // That only applies before the exchange starts. Once Yes has begun,
        // `answer` runs the exchange here, on this thread, and nothing in this
        // window can stop it.
        if !open {
            self.device_check.answer(Consent::Cancel);
            self.device_check_open = false;
            self.device_check_focus = None;
            self.picker.discard_if(|action| {
                matches!(
                    action,
                    PickerAction::SaveReport { .. } | PickerAction::Backup(..)
                )
            });
        } else {
            if let Some(consent) = answer {
                let stage = self.device_check.awaiting();
                self.device_check.answer(consent);
                if stage == Some(Stage::Identification) {
                    self.device_check_step_two_saved = false;
                }
            }
            if let Some(choice) = backup {
                self.start_backup(ctx, choice);
            }
            if verify_g1 || save_report {
                self.request_picker(
                    ctx,
                    PickRequest::SaveWorkbenchFile {
                        name: variax_workbench::probe_report::report_file_name_now(),
                        extension: "txt".into(),
                    },
                    PickerAction::SaveReport { restart: verify_g1 },
                );
            } else if restart {
                if self.device_check.identification_answered() && !self.device_check_step_two_saved
                {
                    self.device_check_notice = device_probe::SAVE_BEFORE_RESTART.into();
                } else {
                    self.restart_device_check();
                }
            }
        }
    }

    /// Write the step 2 report, then start a fresh step 1. A failed or
    /// cancelled save leaves the current check in place.
    fn save_step_two_report_then_restart(&mut self, path: &Path) {
        match self.device_check.save_sanitized_report(path) {
            Ok(()) => {
                self.device_check_step_two_saved = true;
                self.restart_device_check();
                self.device_check_notice = "Steps 1–2 report saved. Step 1 is ready again.".into();
            }
            Err(error) => self.device_check_notice = error.to_string(),
        }
    }
}

const USAGE: &str = "Usage: variax-workbench-gui [FILE | --preset FILE] [--data-dir DIRECTORY] [--artwork EDITOR_IMAGE_FOLDER] [--tab body|pickups|controls] [--display auto|x11|wayland (Linux)] [--screenshot OUTPUT.png]\n       variax-workbench-gui --version";

fn save_screenshot(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    use image::ImageEncoder;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(pixels, width, height, image::ColorType::Rgba8.into())
        .map_err(|e| e.to_string())?;
    variax_workbench::document::save_new(path, &bytes)
}

struct GuiOptions {
    preset: Option<PathBuf>,
    artwork: PathBuf,
    preferences: PathBuf,
    screenshot: Option<PathBuf>,
    tab: Option<&'static str>,
    display: String,
}

fn editor_preferences_path(data_root: Option<&Path>) -> PathBuf {
    data_root
        .map(Path::to_path_buf)
        .unwrap_or_else(default_data_root)
        .join("editor-preferences.txt")
}

#[cfg(not(test))]
fn absolute_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(not(test))]
fn default_data_root() -> PathBuf {
    absolute_env("VARIAX_WORKBENCH_DATA_DIR").unwrap_or_else(|| platform_data_root(absolute_env))
}

#[cfg(test)]
fn default_data_root() -> PathBuf {
    panic!("GUI tests must supply a temporary --data-dir")
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    default_artwork: &Path,
) -> Result<GuiOptions, String> {
    let mut options = GuiOptions {
        preset: None,
        artwork: default_artwork.to_owned(),
        preferences: PathBuf::new(),
        screenshot: None,
        tab: None,
        display: "auto".into(),
    };
    let mut data_root = None;
    let mut artwork = None;
    let mut args = arguments.into_iter();
    while let Some(arg) = args.next() {
        if !arg.starts_with('-') {
            if options.preset.replace(PathBuf::from(arg)).is_some() {
                return Err("Open one file at a time".into());
            }
            continue;
        }
        if !matches!(
            arg.as_str(),
            "--preset" | "--data-dir" | "--artwork" | "--screenshot" | "--tab" | "--display"
        ) {
            return Err(format!("Unknown argument: {arg}"));
        }
        let value = args
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("{arg} requires a value"))?;
        match arg.as_str() {
            "--preset" => {
                if options.preset.replace(PathBuf::from(value)).is_some() {
                    return Err("Open one file at a time".into());
                }
            }
            "--data-dir" => data_root = Some(PathBuf::from(value)),
            "--artwork" => artwork = Some(PathBuf::from(value)),
            "--display" => {
                if !matches!(value.as_str(), "auto" | "x11" | "wayland") {
                    return Err(format!("Unknown display backend: {value}"));
                }
                if !cfg!(target_os = "linux") && value != "auto" {
                    return Err("Explicit X11/Wayland selection is Linux-only".into());
                }
                options.display = value;
            }
            "--screenshot" => {
                let path = PathBuf::from(value);
                if path.extension().and_then(|e| e.to_str()) != Some("png") {
                    return Err("Screenshot output must have a .png extension".into());
                }
                options.screenshot = Some(path);
            }
            "--tab" => {
                options.tab = Some(match value.as_str() {
                    "body" => "body",
                    "pickups" => "pickups",
                    "tuning" => "pickups", // Tuning is always visible.
                    "controls" => "controls",
                    _ => return Err(format!("Unknown editor tab: {value}")),
                });
            }
            _ => unreachable!(),
        }
    }
    options.preferences = editor_preferences_path(data_root.as_deref());
    if let Some(artwork) = artwork.or_else(|| data_root.map(|root| root.join("artwork"))) {
        options.artwork = artwork;
    }
    Ok(options)
}

#[cfg(not(test))]
fn default_artwork() -> PathBuf {
    let installed = default_data_root().join("artwork");
    if absolute_env("VARIAX_WORKBENCH_DATA_DIR").is_some() {
        return installed;
    }
    default_artwork_from_checkout(installed, Path::new(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
fn default_artwork() -> PathBuf {
    default_data_root().join("artwork")
}

fn default_artwork_from_checkout(installed: PathBuf, checkout: &Path) -> PathBuf {
    // A launcher can start the binary from outside the source checkout.
    let extracted = checkout.join("assets/original-ui/modules/variax/editor/images");
    if !installed.is_dir() && extracted.is_dir() {
        return extracted;
    }
    installed
}

#[cfg(not(test))]
fn platform_data_root(absolute_env: impl Fn(&str) -> Option<PathBuf>) -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(path) = absolute_env("LOCALAPPDATA") {
        return path.join("VariaxWorkbench");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    if let Some(path) = absolute_env("XDG_DATA_HOME") {
        return path.join("variax-workbench");
    }
    if let Some(home) = absolute_env("HOME") {
        #[cfg(target_os = "macos")]
        return home.join("Library/Application Support/VariaxWorkbench");
        #[cfg(not(target_os = "macos"))]
        return home.join(".local/share/variax-workbench");
    }
    // This is only an optional artwork lookup; never create or write this path.
    std::env::temp_dir().join("variax-workbench")
}

pub(crate) fn run(title: &str, arguments: impl IntoIterator<Item = String>) -> eframe::Result {
    let arguments: Vec<_> = arguments
        .into_iter()
        .filter(|arg| !cfg!(target_os = "macos") || !arg.starts_with("-psn_"))
        .collect();
    if arguments == ["--version"] {
        println!("{title}");
        return Ok(());
    }
    if arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let arguments = match parse_options(arguments, &default_artwork()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(1);
        }
    };
    let window = arguments
        .screenshot
        .is_none()
        .then(|| WindowPreferences::load(arguments.preferences.with_file_name("window.txt")));
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("variax-workbench")
        .with_inner_size([1120.0, 760.0])
        .with_min_inner_size([960.0, 680.0]);
    if let Some(window) = &window {
        viewport = window.apply(viewport);
    }
    let options = eframe::NativeOptions {
        #[cfg(target_os = "macos")]
        renderer: eframe::Renderer::Glow,
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    let options = {
        let mut options = options;
        let backend = arguments.display.clone();
        options.event_loop_builder = Some(Box::new(move |builder| match backend.as_str() {
            "x11" => {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
            "wayland" => {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                builder.with_wayland();
            }
            _ => {}
        }));
        options
    };
    #[cfg(not(target_os = "linux"))]
    let _ = &arguments.display;
    // Craft Anything saves the launcher's output in its play log.
    eprintln!("Opening {title}");
    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let mut app = Workbench::from_options(&cc.egui_ctx, arguments);
            app.window_preferences = window;
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
use hd500x_panel::HdEditor;
#[cfg(test)]
use podxt_panel::PodxtEditor;
#[cfg(test)]
use variax_panel::{GuitarPending, Tab, VariaxEditor};

#[cfg(test)]
enum Pending {
    Open,
    ReadCurrent,
    Quit,
    QuitPod,
    QuitHd,
    QuitRecording,
}

#[cfg(test)]
impl Workbench {
    fn with_context(
        ctx: &egui::Context,
        preset: Option<PathBuf>,
        artwork: PathBuf,
        panel: Option<&'static str>,
    ) -> Self {
        let mut app = Self::new(
            ctx,
            EditorDefaults {
                artwork,
                preferences: None,
                startup_panel: panel,
            },
        );
        for &family in registry::FAMILIES {
            app.new_tab(family);
        }
        app.select_test_editor::<VariaxEditor>();
        if let Some(path) = preset {
            let editor = app.variax_mut();
            editor.document = None;
            if panel.is_none() {
                editor.tab = Tab::Body;
            }
            editor.load(&path);
        }
        app
    }

    fn test_editor_id<T: FamilyTab>(&self) -> TabId {
        self.tabs
            .entries
            .iter()
            .find_map(|entry| {
                ((entry.editor.as_ref() as &dyn std::any::Any).is::<T>()).then_some(entry.id)
            })
            .expect("fixture tab")
    }

    fn test_editor_is_active<T: FamilyTab>(&self) -> bool {
        self.tabs.active == Some(self.test_editor_id::<T>())
    }

    fn select_test_editor<T: FamilyTab>(&mut self) {
        self.tabs.active = Some(self.test_editor_id::<T>());
    }

    fn editor<T: FamilyTab>(&self) -> &T {
        let id = self.test_editor_id::<T>();
        let any = self.tabs.entries[self.tabs.index(id).unwrap()]
            .editor
            .as_ref() as &dyn std::any::Any;
        any.downcast_ref::<T>().unwrap()
    }

    fn editor_mut<T: FamilyTab>(&mut self) -> &mut T {
        let id = self.test_editor_id::<T>();
        let index = self.tabs.index(id).unwrap();
        let any = self.tabs.entries[index].editor.as_mut() as &mut dyn std::any::Any;
        any.downcast_mut::<T>().unwrap()
    }

    fn variax(&self) -> &VariaxEditor {
        self.editor()
    }
    fn variax_mut(&mut self) -> &mut VariaxEditor {
        self.editor_mut()
    }
    fn podxt(&self) -> &PodxtEditor {
        self.editor()
    }
    fn podxt_mut(&mut self) -> &mut PodxtEditor {
        self.editor_mut()
    }
    fn hd(&self) -> &HdEditor {
        self.editor()
    }
    fn hd_mut(&mut self) -> &mut HdEditor {
        self.editor_mut()
    }

    fn test_pending(&self) -> Option<Pending> {
        if self.recording_close_pending {
            return Some(Pending::QuitRecording);
        }
        if let Some(id) = self.closing_tab {
            return Some(if id == self.test_editor_id::<PodxtEditor>() {
                Pending::QuitPod
            } else if id == self.test_editor_id::<HdEditor>() {
                Pending::QuitHd
            } else {
                Pending::Quit
            });
        }
        match self.variax().pending {
            Some(GuitarPending::Open(_)) => Some(Pending::Open),
            Some(GuitarPending::ReadCurrent) => Some(Pending::ReadCurrent),
            Some(GuitarPending::Quit) => Some(Pending::Quit),
            None => None,
        }
    }

    fn force_test_quit<T: FamilyTab>(&mut self) {
        let id = self.test_editor_id::<T>();
        self.tabs.active = Some(id);
        self.close_request = Some(CloseRequest::Quit {
            approved: Vec::new(),
        });
        self.closing_tab = Some(id);
    }

    fn cancel_test_prompt(&mut self) {
        self.cancel_close();
        self.variax_mut().pending = None;
    }

    fn test_action<T: FamilyTab, R>(
        &mut self,
        f: impl FnOnce(&mut T, &mut EditorServices<'_>) -> R,
    ) -> R {
        let id = self.test_editor_id::<T>();
        let mut result = None;
        self.with_tab(id, |entry, services| {
            let any = entry.editor.as_mut() as &mut dyn std::any::Any;
            result = Some(f(any.downcast_mut::<T>().unwrap(), services));
        });
        result.unwrap()
    }

    fn open_dialog(&mut self) {
        let ctx = egui::Context::default();
        self.test_action(|editor: &mut VariaxEditor, services| editor.open_dialog(&ctx, services));
        self.poll_picker(&ctx);
    }
    fn save_dialog(&mut self) {
        let ctx = egui::Context::default();
        self.test_action(|editor: &mut VariaxEditor, services| editor.save_copy(&ctx, services));
        self.poll_picker(&ctx);
    }
    fn artwork_dialog(&mut self) {
        let ctx = egui::Context::default();
        self.test_action(|editor: &mut VariaxEditor, services| {
            editor.artwork_dialog(&ctx, services)
        });
        self.poll_picker(&ctx);
    }
    fn guitar_workspace(&mut self, ui: &mut egui::Ui) {
        self.test_action(|editor: &mut VariaxEditor, services| {
            editor.guitar_workspace(services, ui)
        });
    }
    fn load_pod_tone(&mut self) {
        self.test_action(PodxtEditor::load_pod_tone);
    }
    fn load_hd_editor_file(&mut self) {
        self.test_action(HdEditor::load_hd_editor_file);
    }
    fn write_hd_copy(&mut self, index: usize) -> bool {
        self.test_action(|editor: &mut HdEditor, services| editor.write_hd_copy(services, index))
    }
}

#[cfg(test)]
mod tests {
    pub(super) use super::recording_panel::tests::prepare_quit_take;
    use super::*;

    pub(super) fn app() -> (egui::Context, Workbench) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let context = egui::Context::default();
        let mut app = Workbench::with_context(
            &context,
            Some(root.join("tests/fixtures/editor-preset.vwm")),
            root.join("assets/original-ui/modules/variax/editor/images"),
            None,
        );
        // These tests exercise egui directly, without a display or theme polling.
        app.theme_polled = Instant::now();
        (context, app)
    }

    pub(super) fn prompt_frame(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| {
                app.poll_picker(ctx);
                app.pending_dialog(ctx);
            },
        );
    }

    pub(super) fn click_prompt(context: &egui::Context, app: &mut Workbench, action: &'static str) {
        // Let a newly created or resized centered prompt settle before clicking.
        prompt_frame(context, app, vec![]);
        prompt_frame(context, app, vec![]);
        let pos = app.pending_buttons[action].center();
        prompt_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        prompt_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        app.poll_picker(context);
    }

    pub(super) fn recording_frame(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        // Tall enough for the PODxt Live pedalboard plus the controls under it.
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 8000.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    pub(super) fn recording_frame_tall(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 8000.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn click_device_check(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        if id == "open" {
            click_toolbar_tools(context, app);
        }
        recording_frame(context, app, vec![]);
        recording_frame(context, app, vec![]);
        click_recording_frame(context, app, app.device_check_buttons[id].center());
    }

    pub(super) fn click_toolbar_tools(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        recording_frame(context, app, vec![]);
        click_recording_frame(context, app, app.recording_buttons["tools-menu"].center());
    }

    fn click_recording_frame(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        recording_frame(context, app, vec![]);
    }

    fn device_check_app() -> (egui::Context, Workbench) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        click_device_check(&context, &mut app, "open");
        assert!(app.device_check_open);
        (context, app)
    }

    fn scripted_device_check_app(scenario: device_probe::Scenario) -> (egui::Context, Workbench) {
        let (context, mut app) = device_check_app();
        app.device_check = ProbeSession::new(RigBackend::Simulated(
            device_probe::SimulatedRig::new(scenario),
        ));
        app.device_check_focus = None;
        // Let the injected session draw its consent buttons and focus No
        // before sending the first keyboard event.
        recording_frame(&context, &mut app, vec![]);
        (context, app)
    }

    #[cfg(target_os = "linux")]
    fn inject_device_check_tree(app: &mut Workbench, tree: &tempfile::TempDir) {
        app.device_check = ProbeSession::new(RigBackend::ThisComputer(HostRig::new(
            LinuxSysfs::with_root(tree.path()),
        )));
        app.device_check_focus = None;
    }

    #[test]
    fn device_check_save_failure_is_shown_and_does_not_rerun() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        click_device_check(&context, &mut app, "yes");
        click_device_check(&context, &mut app, "yes");
        let calls = app.device_check.backend().calls();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SECRET-USER-report.txt");
        app.note_sanitized_report_save(&path);
        app.note_sanitized_report_save(&path);
        recording_frame(&context, &mut app, vec![]);
        assert!(
            app.device_check_visible
                .contains("report with that name already exists")
        );
        assert!(!app.device_check_visible.contains("SECRET"));
        assert!(!app.device_check_notice.contains("SECRET"));
        assert_eq!(app.device_check.backend().calls(), calls);
    }

    #[test]
    fn device_check_window_opens_on_this_computer_and_declines_by_default() {
        let (context, mut app) = device_check_app();
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        if let Some(notice) = device_check_platform_notice() {
            assert_eq!(app.device_check_visible, notice);
            assert!(app.device_check.finished());
            assert!(!app.device_check_buttons.contains_key("yes"));
            assert!(!app.device_check_buttons.contains_key("no"));
            assert_eq!(app.device_check.backend().calls(), Default::default());
            return;
        }
        assert!(
            app.device_check_visible
                .contains(device_probe::this_computer_label())
        );
        assert!(!app.device_check_visible.contains("Simulation"));
        assert!(
            app.device_check_visible
                .contains(&device_probe::availability_banner())
        );

        // Rendering alone never runs a step.
        for _ in 0..5 {
            recording_frame(&context, &mut app, vec![]);
        }
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert!(app.device_check.locked(Stage::ChangeCheck));
        if device_probe::live_stage_available(Stage::UsbRecognition) {
            assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
            assert!(
                app.device_check_visible
                    .contains(Stage::UsbRecognition.action())
            );
            assert!(
                app.device_check_visible
                    .contains(Stage::UsbRecognition.risk())
            );
            press_device_check_enter(&context, &mut app);
            assert_eq!(
                app.device_check.status(Stage::UsbRecognition),
                StageStatus::Stopped
            );
            assert!(app.device_check_visible.contains("Stopped at step 1"));
        } else {
            assert!(app.device_check.finished());
            assert!(!app.device_check_buttons.contains_key("yes"));
            assert!(!app.device_check_visible.contains("real device: open"));
        }
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert!(app.variax().read_worker.is_none());
    }

    fn press_device_check_enter(context: &egui::Context, app: &mut Workbench) {
        recording_frame(
            context,
            app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        recording_frame(context, app, vec![]);
    }

    #[test]
    fn device_check_defaults_to_no_on_each_new_consent() {
        if let Some(notice) = device_check_platform_notice() {
            let (context, mut app) = device_check_app();
            press_device_check_enter(&context, &mut app);
            assert_eq!(app.device_check_visible, notice);
            for stage in Stage::LADDER {
                assert_eq!(app.device_check.status(stage), StageStatus::NotRun);
            }
            assert_eq!(app.device_check.backend().calls(), Default::default());
        }

        for index in 0..3 {
            let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
            for _ in 0..index {
                recording_frame(&context, &mut app, vec![]);
                let yes_id = app.device_check_button_ids["yes"];
                context.memory_mut(|memory| memory.request_focus(yes_id));
                press_device_check_enter(&context, &mut app);
            }
            let stage = Stage::LADDER[index];
            assert_eq!(app.device_check.awaiting(), Some(stage));
            let calls = app.device_check.backend().calls();
            press_device_check_enter(&context, &mut app);
            assert_eq!(app.device_check.status(stage), StageStatus::Stopped);
            assert!(app.device_check_visible.contains("You chose No"));
            assert_eq!(app.device_check.backend().calls(), calls);
        }
    }

    #[test]
    fn device_check_scrolls_to_the_report_on_a_short_viewport() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        let frame = |app: &mut Workbench, height, events| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, height))),
                    events,
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        };
        // Layout the shorter viewport before injecting a wheel event.
        frame(&mut app, 330.0, vec![]);
        frame(&mut app, 330.0, vec![]);
        assert!(app.device_check_buttons["save-report"].bottom() > 330.0);
        let window = context
            .memory(|memory| memory.area_rect(egui::Id::new("Device check")))
            .unwrap();
        let pos = window.center();
        for _ in 0..12 {
            frame(
                &mut app,
                330.0,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -500.0),
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let report = app.device_check_buttons["save-report"];
        assert!(
            window.contains_rect(report),
            "{window:?} does not contain {report:?}"
        );
        assert!(
            report.bottom() <= 330.0,
            "{report:?} is outside the viewport"
        );
        frame(&mut app, 1200.0, vec![]);
        frame(&mut app, 1200.0, vec![]);
        let expanded = context
            .memory(|memory| memory.area_rect(egui::Id::new("Device check")))
            .unwrap();
        assert!(expanded.height() > window.height());
        assert!(expanded.contains_rect(app.device_check_buttons["restart"]));
        assert!(expanded.contains_rect(app.device_check_buttons["save-report"]));
        assert_eq!(app.device_check.backend().calls(), Default::default());
    }

    #[test]
    fn device_check_timeout_keeps_save_and_retry_visible_on_a_short_viewport() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        app.device_check = ProbeSession::new(RigBackend::Simulated(
            device_probe::SimulatedRig::with_identity(
                device_probe::Scenario::Connected,
                variax_workbench::identity_stage::happy_script().silent(),
            ),
        ));
        click_device_check(&context, &mut app, "yes");
        click_device_check(&context, &mut app, "yes");
        assert!(app.device_check.identification_timed_out());
        let calls = app.device_check.backend().calls();
        assert_eq!(calls.identity_requests, 1);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("step2-timeout.txt");
        app.dialogs = Box::new(FakeDialogs {
            save: [None, Some(path.clone())].into(),
            ..FakeDialogs::default()
        });
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 330.0));
        let frame = |app: &mut Workbench, events| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    events,
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        };
        let click_retry = |app: &mut Workbench| {
            frame(app, vec![]);
            frame(app, vec![]);
            let retry = app.device_check_buttons["verify-g1"];
            assert!(viewport.contains_rect(retry), "{retry:?}");
            assert!(viewport.contains_rect(app.device_check_buttons["save-report"]));
            let pos = retry.center();
            for pressed in [true, false] {
                frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            frame(app, vec![]);
            frame(app, vec![]);
        };

        click_retry(&mut app);
        assert!(app.device_check_notice.contains("Report save canceled"));
        assert!(app.device_check.identification_timed_out());
        assert_eq!(app.device_check.backend().calls(), calls);
        assert!(!path.exists());

        click_retry(&mut app);
        let report = std::fs::read_to_string(path).unwrap();
        assert!(report.contains("stop=timed-out"), "{report}");
        assert!(report.contains("cleanup=ok"), "{report}");
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        frame(&mut app, vec![]);
        assert!(viewport.contains_rect(app.device_check_buttons["no"]));
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(app.device_check_button_ids["no"]),
        );
    }

    #[test]
    fn device_check_decline_button_stops_without_running() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        click_device_check(&context, &mut app, "no");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check.finished());
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert!(app.device_check_visible.contains("You chose No"));
    }

    #[test]
    fn device_check_passed_step_does_not_advance_by_itself() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Passed
        );
        assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
        for _ in 0..5 {
            recording_frame(&context, &mut app, vec![]);
        }
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::NotRun
        );
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        let marker = if device_probe::live_stage_available(Stage::Identification) {
            "open"
        } else {
            "unavailable"
        };
        assert!(app.device_check_visible.contains(&format!(
            "step 2 waiting for your answer real device: {marker}"
        )));
        assert!(app.device_check_visible.contains("0e41:4650"));
        for _ in 0..3 {
            click_device_check(&context, &mut app, "yes");
        }
        for stage in Stage::LADDER {
            assert_eq!(app.device_check.status(stage), StageStatus::Passed);
        }
        assert!(app.device_check.finished());
        assert!(matches!(
            app.device_check.backend(),
            RigBackend::Simulated(_)
        ));
    }

    #[test]
    fn device_check_stops_on_failure_and_start_over_resets() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::TwoPods);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check.finished());
        assert!(app.device_check_visible.contains("2 PODxt Live devices"));
        assert!(!app.device_check_visible.contains("waiting for your answer"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
    }

    /// A temporary fake `/sys` + `/proc` tree with `units` PODxt Live devices.
    /// Tests never point the reader anywhere else.
    #[cfg(target_os = "linux")]
    fn fake_host_tree(units: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let write = |path: PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let devices = root.path().join("sys/bus/usb/devices");
        for name in units {
            let base = devices.join(name);
            write(base.join("idVendor"), "0e41\n");
            write(base.join("idProduct"), "4650\n");
            write(base.join("bcdDevice"), "0001\n");
            write(base.join("product"), "PODxt Live\n");
            write(base.join("serial"), "SECRET-SERIAL\n");
            for (number, driver) in [(0, "snd-usb-pod"), (1, "snd-usb-variax")] {
                let interface = devices.join(format!("{name}:1.{number}"));
                write(
                    interface.join("bInterfaceNumber"),
                    &format!("{number:02x}\n"),
                );
                std::fs::create_dir_all(root.path().join("sys/bus/usb/drivers").join(driver))
                    .unwrap();
                std::os::unix::fs::symlink(
                    format!("../../../bus/usb/drivers/{driver}"),
                    interface.join("driver"),
                )
                .unwrap();
            }
        }
        std::fs::create_dir_all(&devices).unwrap();
        write(
            root.path().join("proc/asound/cards"),
            " 0 [PODxtLive      ]: Line6-POD - PODxt Live\n                      Line 6 PODxt Live at USB 1-2:1.0\n",
        );
        root
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_this_computer_runs_only_step_one_on_an_injected_tree() {
        assert!(device_probe::live_stage_available(Stage::UsbRecognition));
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        let RigBackend::ThisComputer(rig) = app.device_check.backend() else {
            panic!("expected This computer mode");
        };
        assert_eq!(rig.reader().root(), tree.path());
        // Opening the window reads nothing; step 1 waits at the default No.
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert!(
            app.device_check_visible
                .contains(&device_probe::availability_banner())
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::this_computer_label())
        );
        if device_probe::live_stage_available(Stage::Identification) {
            assert!(
                app.device_check_visible
                    .contains("step 2 not run real device: open")
            );
        } else {
            assert!(
                app.device_check_visible
                    .contains("step 2 unavailable in this build")
            );
        }
        if device_probe::live_stage_available(Stage::SettingsRead) {
            assert!(
                app.device_check_visible
                    .contains("step 3 not run real device: open"),
                "step 3: {}",
                app.device_check_visible
            );
        } else {
            assert!(
                app.device_check_visible
                    .contains("step 3 unavailable in this build"),
                "step 3: {}",
                app.device_check_visible
            );
        }
        assert!(
            app.device_check_visible
                .contains("step 4 unavailable in this build"),
            "step 4: {}",
            app.device_check_visible
        );
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Passed
        );
        // A usb build offers step 2. Answer No. Do not send the identity request.
        if device_probe::live_stage_available(Stage::Identification) {
            assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
            assert!(app.device_check.g1_expected_binding());
            assert!(
                app.device_check_visible
                    .contains(device_probe::STEP_TWO_YES)
            );
            click_device_check(&context, &mut app, "no");
            assert_eq!(
                app.device_check.status(Stage::Identification),
                StageStatus::Stopped
            );
            assert!(app.device_check_visible.contains("Stopped at step 2"));
            assert!(device_probe::reports_panel_note().contains("steps 1–3"));
        } else {
            assert_eq!(app.device_check.awaiting(), None);
            assert!(!app.device_check_buttons.contains_key("yes"));
            assert!(!app.device_check_buttons.contains_key("no"));
            assert_eq!(
                app.device_check.status(Stage::Identification),
                StageStatus::NotRun
            );
            assert!(app.device_check.locked(Stage::Identification));
            assert!(app.device_check_visible.contains("Step 1 finished"));
            assert!(
                app.device_check_visible
                    .contains(device_probe::this_computer_finished_message())
            );
            assert!(device_probe::reports_panel_note().contains("This build offers step 1."));
        }
        assert_eq!(
            app.device_check.status(Stage::SettingsRead),
            StageStatus::NotRun
        );
        assert_eq!(
            app.device_check.locked(Stage::SettingsRead),
            !device_probe::live_stage_available(Stage::SettingsRead)
        );
        assert_eq!(
            app.device_check.status(Stage::ChangeCheck),
            StageStatus::NotRun
        );
        assert!(app.device_check.locked(Stage::ChangeCheck));
        let calls = app.device_check.backend().calls();
        assert_eq!(
            (
                calls.os_reads,
                calls.identity_requests,
                calls.settings_reads
            ),
            (1, 0, 0)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::reports_panel_note())
        );
        assert!(app.device_check_visible.contains("0e41:4650"));
        let report = app.device_check.report();
        assert!(report.contains(&format!(
            "mode=this-computer {}",
            device_probe::open_gates().token()
        )));
        assert!(report.contains("interface 1 driver snd-usb-variax"));
        assert!(!report.contains("SECRET"), "serial number leaked");
        assert!(
            !report.contains(&*tree.path().to_string_lossy()),
            "path leaked"
        );
        assert!(app.variax().read_worker.is_none());
    }

    #[cfg(all(target_os = "linux", feature = "usb"))]
    #[test]
    fn device_check_backup_asks_before_it_reads() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        assert!(app.device_check_visible.contains(BACKUP_VARIAX));
        assert!(app.device_check_visible.contains(BACKUP_PODXT));
        assert!(
            app.device_check_visible
                .contains("step 4 unavailable in this build")
        );
        click_device_check(&context, &mut app, "backup-variax");
        assert!(app.device_check_visible.contains(BACKUP_VARIAX_DOES));
        click_device_check(&context, &mut app, "backup-no");
        assert!(app.device_check_notice.contains("Nothing was read"));
        assert!(app.backup_rx.is_none());
        click_device_check(&context, &mut app, "backup-podxt");
        assert!(app.device_check_visible.contains(BACKUP_PODXT_DOES));
        app.dialogs = Box::new(FakeDialogs {
            folder: [Some(tree.path().to_owned())].into(),
            ..Default::default()
        });
        click_device_check(&context, &mut app, "backup-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert!(app.backup_rx.is_none());
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        assert_eq!(app.device_check.backend().calls().settings_reads, 0);
    }

    #[test]
    fn device_check_cancelled_or_discarded_picker_never_starts_backup() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        app.dialogs = Box::new(FakeDialogs {
            folder: [None, Some(PathBuf::from("ignored-root"))].into(),
            ..FakeDialogs::default()
        });
        app.start_backup(&context, BackupChoice::Variax);
        assert!(app.picker.is_open());
        app.poll_picker(&context);
        assert_eq!(
            app.device_check_notice,
            "No folder chosen. Nothing was sent."
        );
        assert!(app.backup_rx.is_none());

        app.start_backup(&context, BackupChoice::Variax);
        let window = context
            .memory(|memory| memory.area_rect(egui::Id::new("Device check")))
            .unwrap();
        let close = Pos2::new(window.right() - 20.0, window.top() + 18.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: close,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        recording_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(close), click(true), click(false)],
        );
        assert!(!app.device_check_open);
        assert_eq!(
            app.device_check_notice,
            "No folder chosen. Nothing was sent."
        );
        assert!(app.backup_rx.is_none());
        assert!(!app.picker.is_open());
    }

    #[cfg(all(target_os = "linux", feature = "usb"))]
    #[test]
    fn device_check_tone_actions_ask_before_they_send() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        assert!(app.device_check_visible.contains(TONE_TRY));
        assert!(app.device_check_visible.contains(TONE_CHANNEL));
        assert!(app.device_check_visible.contains(TONE_KEEP));
        assert!(
            app.device_check_visible
                .contains("step 4 unavailable in this build")
        );
        click_device_check(&context, &mut app, "tone-try");
        assert!(app.device_check_visible.contains(TONE_TRY_DOES));
        click_device_check(&context, &mut app, "tone-no");
        assert!(app.device_check_notice.contains("Nothing was changed"));
        assert!(app.tone_rx.is_none());
        click_device_check(&context, &mut app, "tone-try");
        click_device_check(&context, &mut app, "tone-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert!(app.tone_rx.is_none());
        app.tone_channel = "nope".into();
        click_device_check(&context, &mut app, "tone-channel");
        assert!(app.device_check_notice.contains("1A"));
        assert!(app.tone_choice.is_none());
        app.tone_channel = "2C".into();
        click_device_check(&context, &mut app, "tone-channel");
        assert!(app.device_check_visible.contains("2C"));
        click_device_check(&context, &mut app, "tone-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        assert_eq!(app.device_check.backend().calls().settings_reads, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_this_computer_decline_and_two_units_stop() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        click_device_check(&context, &mut app, "no");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let two = fake_host_tree(&["1-2", "1-3"]);
        inject_device_check_tree(&mut app, &two);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check_visible.contains("2 PODxt Live devices"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
    }

    /// Enter on the focused Yes button is a real Yes. The close regression
    /// below uses that same key, so a zero-read result means Close won.
    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_enter_on_focused_yes_reads_the_injected_tree() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        let yes_id = app.device_check_button_ids["yes"];
        context.memory_mut(|memory| memory.request_focus(yes_id));
        recording_frame(
            &context,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.device_check.backend().calls().os_reads, 1);
        assert!(app.device_check.report().contains("decision=yes"));
    }

    /// Yes and Close in one frame go through `device_check_window`'s dispatch.
    /// Close is handled first, so the injected reader is never called.
    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_close_in_the_same_frame_as_yes_reads_nothing() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        inject_device_check_tree(&mut app, &tree);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let yes_id = app.device_check_button_ids["yes"];
        context.memory_mut(|memory| memory.request_focus(yes_id));
        let window = context
            .memory(|memory| memory.area_rect(egui::Id::new("Device check")))
            .expect("device check window");
        let close_at = Pos2::new(window.right() - 20.0, window.top() + 18.0);
        recording_frame(
            &context,
            &mut app,
            vec![
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(close_at),
                egui::Event::PointerButton {
                    pos: close_at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: close_at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(!app.device_check_open, "closing the window must win");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert_eq!(app.device_check.awaiting(), None);
        let report = app.device_check.report();
        assert!(report.contains("decision=cancel"), "{report}");
        assert!(!report.contains("decision=yes"), "{report}");
        assert_eq!(
            app.device_check.backend().calls(),
            Default::default(),
            "the injected reader must record zero calls"
        );
        assert!(app.variax().read_worker.is_none());
    }

    #[test]
    fn consent_preflight_step_two_screen_saves_before_g1() {
        let (context, mut app) = scripted_device_check_app(device_probe::Scenario::Connected);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
        assert!(app.device_check.g1_expected_binding());
        assert!(
            app.device_check_visible
                .contains(device_probe::STEP_TWO_YES),
            "{}",
            app.device_check_visible
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::CANCEL_LIMIT)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::APP_REQUEST_VERSUS_LINUX)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::STEP_TWO_PREFLIGHT)
        );
        assert!(app.device_check_buttons.contains_key("no"));
        assert_eq!(device_probe::DEFAULT_CONSENT, device_probe::Consent::No);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::Passed
        );
        assert!(app.device_check.identification_answered());
        assert!(
            app.device_check_visible
                .contains(device_probe::SAVE_THEN_CHECK_AGAIN)
        );
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("simulation");
        };
        assert_eq!(rig.identity_calls(), 8);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::Passed
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::SAVE_BEFORE_RESTART),
            "{}",
            app.device_check_visible
        );
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("simulation");
        };
        assert_eq!(rig.identity_calls(), 8);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("step2-report.txt");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..FakeDialogs::default()
        });
        click_device_check(&context, &mut app, "verify-g1");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            saved.contains(device_probe::open_gates().token()),
            "{saved}"
        );
        assert!(saved.contains("guitar=variax-600-3.10"), "{saved}");
        assert!(saved.contains("stage=2"), "{saved}");
        assert!(!saved.contains("SECRET"));
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert!(!app.device_check.g1_expected_binding());
        assert!(!app.device_check.identification_answered());
        assert!(app.device_check_visible.contains("Step 1 is ready again."));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("fresh simulation");
        };
        assert_eq!(rig.identity_calls(), 0);
    }

    #[test]
    fn invalid_arguments_return_errors_instead_of_panicking() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        for args in [
            vec!["--preset"],
            vec!["--artwork", "--preset"],
            vec!["--unknown"],
            vec!["--tab", "invalid"],
            vec!["--screenshot", "image.jpg"],
            vec!["--display", "unknown"],
        ] {
            assert!(parse_options(args.into_iter().map(str::to_owned), root).is_err());
        }
        let options = parse_options(
            [
                "--tab",
                "controls",
                "--preset",
                "model.vwm",
                "--screenshot",
                "preview.png",
                "--data-dir",
                root.to_str().unwrap(),
            ]
            .into_iter()
            .map(str::to_owned),
            root,
        )
        .unwrap();
        assert_eq!(options.tab, Some("controls"));
        assert_eq!(options.preset.as_deref(), Some(Path::new("model.vwm")));
    }

    #[test]
    fn fresh_install_opens_the_example_without_assets_and_can_open_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let options = parse_options(
            [
                "--data-dir".to_owned(),
                directory.path().display().to_string(),
            ],
            &directory.path().join("artwork"),
        )
        .unwrap();
        assert!(options.preset.is_none());
        let context = egui::Context::default();
        let mut app =
            Workbench::with_context(&context, options.preset, options.artwork, options.tab);
        app.theme_polled = Instant::now();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.variax().error.is_none());
        assert_eq!(app.variax().tab, Tab::Pickups);
        assert_eq!(
            app.variax().document.as_ref().unwrap().preset().name(),
            "Spank example"
        );
        assert!(app.variax().read_worker.is_none());
        let textures = &app.variax().textures;
        assert!(textures.contains_key("bodies/strat.png"));
        assert!(textures.contains_key("pickups/strat.png"));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(file.clone())].into(),
            ..Default::default()
        });
        app.open_dialog();
        assert!(app.variax().error.is_none());
        let before = fs::read(file).unwrap();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.variax().error.is_none());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.variax().dirty());
    }

    #[test]
    fn startup_honors_the_requested_file_and_tab() {
        let directory = tempfile::tempdir().unwrap();
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
        let artwork = directory.path().join("artwork");
        let context = egui::Context::default();

        let example = Workbench::with_context(&context, None, artwork.clone(), Some("controls"));
        assert_eq!(example.variax().tab, Tab::Controls);
        assert_eq!(
            example.variax().document.as_ref().unwrap().preset().name(),
            "Spank example"
        );

        let supplied = Workbench::with_context(&context, Some(file.clone()), artwork.clone(), None);
        assert_eq!(supplied.variax().tab, Tab::Body);
        assert_eq!(
            supplied.variax().document.as_ref().unwrap().as_bytes(),
            fs::read(file).unwrap()
        );

        let missing = directory.path().join("missing.vwm");
        let failed = Workbench::with_context(&context, Some(missing), artwork, None);
        assert!(failed.variax().document.is_none());
        assert!(
            failed
                .variax()
                .error
                .as_deref()
                .unwrap()
                .contains("missing.vwm")
        );
    }

    #[test]
    fn desktop_file_argument_and_explicit_artwork_override_data_directory() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let chosen = root.to_str().unwrap();
        let parse = |args: &[&str]| parse_options(args.iter().map(|s| (*s).to_owned()), root);
        let options = parse(&["a preset.vwm", "--data-dir", chosen]).unwrap();
        assert_eq!(options.preset.as_deref(), Some(Path::new("a preset.vwm")));
        assert_eq!(options.artwork, root.join("artwork"));
        for args in [
            vec!["--artwork", "images", "--data-dir", chosen],
            vec!["--data-dir", chosen, "--artwork", "images"],
        ] {
            assert_eq!(parse(&args).unwrap().artwork, Path::new("images"));
        }
        assert!(parse(&["one.vwm", "two.vwm"]).is_err());
        assert!(parse(&["one.vwm", "--preset", "two.vwm"]).is_err());
        assert!(parse(&["--data-dir"]).is_err());
    }

    #[test]
    #[should_panic(expected = "GUI tests must supply a temporary --data-dir")]
    fn tests_refuse_to_resolve_the_real_data_root() {
        let directory = tempfile::tempdir().unwrap();
        let _ = parse_options(Vec::new(), directory.path());
    }

    #[test]
    fn artwork_lookup_uses_the_checkout_when_the_installed_folder_is_absent() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("user-data/artwork");
        let checkout = directory.path().join("source checkout");
        let extracted = checkout.join("assets/original-ui/modules/variax/editor/images");
        let resolve = || default_artwork_from_checkout(installed.clone(), &checkout);

        assert_eq!(resolve(), installed);
        assert!(!installed.exists());
        fs::create_dir_all(&extracted).unwrap();
        assert_eq!(resolve(), extracted);
        fs::create_dir_all(&installed).unwrap();
        assert_eq!(resolve(), installed);
    }

    #[test]
    fn screenshot_output_creates_a_png_and_never_replaces_existing_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.png");
        save_screenshot(&path, &[0, 0, 0, 255], 1, 1).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(original.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(save_screenshot(&path, &[255, 0, 0, 255], 1, 1).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

#[cfg(test)]
#[path = "app/shell/tests.rs"]
mod shell_tests;

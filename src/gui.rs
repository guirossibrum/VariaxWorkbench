//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use variax_workbench::pod_audio::RecordRoute;
use variax_workbench::pod_preset::{self, Library};
use variax_workbench::recording::{self, ClockSelection};
use variax_workbench::rig::{Rejection, RoleKind, Session};
use variax_workbench::{document::EditorDocument, preset::PresetFile};

const PICKUP1: u32 = 0x02010001;
const PICKUP2: u32 = 0x02010002;
const BODY: u32 = 0x02010003;
const WIRING: u32 = 0x02010004;
const TUNING: u32 = 0x02010005;
const LIVE_ACCESS_LOCKED: bool = true;

#[derive(Clone, Deserialize)]
struct ModelArt {
    kind: String,
    id: u32,
    name: String,
    image: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tab {
    Body,
    Pickups,
    Tuning,
    Controls,
}
enum Pending {
    Open(PathBuf),
    ReadCurrent,
    Quit,
    /// Quit while at least one PODxt Live tone has an applied unsaved edit.
    QuitPod,
    QuitRecording,
}

#[derive(Clone, Copy)]
enum PodMembershipAction {
    Duplicate,
    Remove,
    MoveUp,
    MoveDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingStompHarmony {
    Harmony1M335(pod_preset::StompHarmony1M335Selection),
    Harmony1457(pod_preset::StompHarmony1457Selection),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingDelayEdit {
    Note(pod_preset::DelayNoteChoice),
    TimeMs(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingModEdit {
    Note(pod_preset::ModNoteChoice),
    SpeedHz(f32),
}

trait FileDialogs {
    fn open(&mut self) -> Option<PathBuf>;
    fn save(&mut self, suggested: &str, extension: &str) -> Option<PathBuf>;
    fn artwork(&mut self) -> Option<PathBuf>;
    fn open_wav(&mut self) -> Option<PathBuf>;
    fn save_wav(&mut self, suggested: &str) -> Option<PathBuf>;
    fn save_aiff(&mut self, suggested: &str) -> Option<PathBuf>;
    fn open_pod_tone(&mut self) -> Option<PathBuf>;
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf>;
    #[cfg(test)]
    fn queue_pod_save(&mut self, _path: Option<PathBuf>) {
        panic!("POD save queue is available only on fake dialogs");
    }
}

#[cfg(not(test))]
struct NativeDialogs;
#[cfg(not(test))]
impl FileDialogs for NativeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(
                "Workbench model, bank or collection",
                &["vwm", "vwb", "l6c"],
            )
            .pick_file()
    }
    fn save(&mut self, suggested: &str, extension: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Workbench document", &[extension])
            .set_file_name(suggested)
            .save_file()
    }
    fn artwork(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }
    fn open_wav(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("PCM WAV", &["wav"])
            .pick_file()
    }
    fn save_wav(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("PCM WAV", &["wav"])
            .set_file_name(suggested)
            .save_file()
    }
    fn save_aiff(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Exact-clock AIFF", &["aiff", "aif"])
            .set_file_name(suggested)
            .save_file()
    }
    fn open_pod_tone(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_file()
    }
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_file_name(suggested).save_file()
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeDialogs {
    open: std::collections::VecDeque<Option<PathBuf>>,
    save: std::collections::VecDeque<Option<PathBuf>>,
    artwork: std::collections::VecDeque<Option<PathBuf>>,
    wav: std::collections::VecDeque<Option<PathBuf>>,
    pod_open: std::collections::VecDeque<Option<PathBuf>>,
    pod_save: std::collections::VecDeque<Option<PathBuf>>,
}
#[cfg(test)]
impl FileDialogs for FakeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        self.open.pop_front().expect("unexpected open dialog")
    }
    fn save(&mut self, _: &str, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected save dialog")
    }
    fn artwork(&mut self) -> Option<PathBuf> {
        self.artwork.pop_front().expect("unexpected artwork dialog")
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
    fn queue_pod_save(&mut self, path: Option<PathBuf>) {
        self.pod_save.push_back(path);
    }
}

fn suggested_tone_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| if ch == '/' || ch == '\\' { '-' } else { ch })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "podxt-live-tone".into()
    } else {
        trimmed.to_owned()
    }
}

fn rejection_text(rejection: &Rejection) -> String {
    match rejection {
        Rejection::Format { detail } => detail.clone(),
        Rejection::CrossFamily { .. } => {
            "that file is a Variax tone, not a PODxt Live preset".into()
        }
        Rejection::Unsupported { .. } => "this profile cannot open a PODxt Live tone".into(),
        Rejection::Locked { .. } => "that operation stays locked".into(),
        other => format!("{other:?}"),
    }
}

fn route_button_id(route: RecordRoute) -> &'static str {
    match route {
        RecordRoute::SendProcessedGuitar => "route-processed",
        RecordRoute::SendCleanGuitar => "route-clean",
        RecordRoute::SendCleanGuitarReampPlayback => "route-reamp-clean",
        RecordRoute::SendReampPlayback => "route-reamp",
    }
}

fn clock_button_id(clock: ClockSelection) -> &'static str {
    match clock {
        ClockSelection::Unselected => "clock-none",
        ClockSelection::Integer(44_100) => "clock-44100",
        ClockSelection::Integer(48_000) => "clock-48000",
        ClockSelection::Integer(_) => "clock-other",
        ClockSelection::NativePodxt => "clock-native",
    }
}

struct Workbench {
    dialogs: Box<dyn FileDialogs>,
    document: Option<EditorDocument>,
    catalog: Vec<ModelArt>,
    artwork: PathBuf,
    textures: HashMap<String, TextureHandle>,
    missing_textures: HashSet<String>,
    error: Option<String>,
    status: String,
    tab: Tab,
    help: bool,
    pending: Option<Pending>,
    permit_close: bool,
    drag_origin: Option<(u32, f32)>,
    theme_polled: Instant,
    theme_text: String,
    screenshot: Option<PathBuf>,
    frame_count: u32,
    recording_open: bool,
    recorder: recording::Panel,
    pod_open: bool,
    pod_library: Library,
    pod_bundle: Option<pod_preset::PodBundleLibrary>,
    pod_selected: Option<usize>,
    pod_cab_pending: Option<(usize, pod_preset::PodCabModelChoice)>,
    pod_delay_model_pending: Option<(usize, pod_preset::PodDelayModelChoice)>,
    pod_mod_model_pending: Option<(usize, pod_preset::PodModModelChoice)>,
    pod_harmony_pending: Option<(usize, pod_preset::StompHarmonyControl, PendingStompHarmony)>,
    pod_delay_pending: Option<(usize, PendingDelayEdit)>,
    pod_mod_pending: Option<(usize, PendingModEdit)>,
    pod_name: String,
    pod_message: String,
    #[cfg(test)]
    bank_rows: HashMap<(usize, usize), Rect>,
    #[cfg(test)]
    pending_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    pickup_slider: Option<Rect>,
    #[cfg(test)]
    recording_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    recording_visible: String,
    #[cfg(test)]
    recording_capture_attempts: u32,
    #[cfg(test)]
    pod_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    pod_rows: HashMap<usize, Rect>,
    #[cfg(test)]
    pod_cab_combo: Option<Rect>,
    #[cfg(test)]
    pod_cab_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_drive_slider: Option<Rect>,
    #[cfg(test)]
    pod_stomp_sliders: HashMap<pod_preset::StompControl, Rect>,
    #[cfg(test)]
    pod_whammy_knob_sliders: HashMap<pod_preset::StompWhammyKnob, Rect>,
    #[cfg(test)]
    pod_eq_frequency_sliders: HashMap<pod_preset::PodxtEqBand, Rect>,
    #[cfg(test)]
    pod_eq_gain_sliders: HashMap<pod_preset::PodxtEqBand, Rect>,
    #[cfg(test)]
    pod_model_enable_checks: HashMap<pod_preset::PodModelEnable, Rect>,
    #[cfg(test)]
    pod_model_enable_menu: Option<Rect>,
    #[cfg(test)]
    pod_mod_sliders: HashMap<pod_preset::ModControl, Rect>,
    #[cfg(test)]
    pod_reverb_sliders: HashMap<pod_preset::ReverbControl, Rect>,
    #[cfg(test)]
    pod_delay_sliders: HashMap<pod_preset::DelayControl, Rect>,
    #[cfg(test)]
    pod_delay_note_options: HashMap<pod_preset::DelayNoteChoice, Rect>,
    #[cfg(test)]
    pod_delay_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_delay_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_mod_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_mod_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_mod_note_options: HashMap<pod_preset::ModNoteChoice, Rect>,
    #[cfg(test)]
    pod_delay_time_slider: Option<Rect>,
    #[cfg(test)]
    pod_mod_speed_slider: Option<Rect>,
    #[cfg(test)]
    pod_delay_heads_choices: HashMap<pod_preset::DelayHeadsSelection, Rect>,
    #[cfg(test)]
    pod_delay_bits_choices: HashMap<pod_preset::DelayBitsSelection, Rect>,
    #[cfg(test)]
    pod_stomp_wave_choices: HashMap<pod_preset::StompWaveSelection, Rect>,
    #[cfg(test)]
    pod_harmony1_choices: HashMap<pod_preset::StompHarmony1M335Selection, Rect>,
    #[cfg(test)]
    pod_harmony2_choices: HashMap<pod_preset::StompHarmony1457Selection, Rect>,
    #[cfg(test)]
    pod_visible: String,
    read_receiver:
        Option<std::sync::mpsc::Receiver<Result<variax_workbench::usb::CurrentPreset, String>>>,
    read_worker: Option<(
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    )>,
}

impl Workbench {
    fn new(cc: &eframe::CreationContext<'_>, preset: Option<PathBuf>, artwork: PathBuf) -> Self {
        Self::with_context(&cc.egui_ctx, preset, artwork)
    }

    fn with_context(ctx: &egui::Context, preset: Option<PathBuf>, artwork: PathBuf) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.visuals.window_corner_radius = 0.into();
        style.visuals.widgets.inactive.corner_radius = 0.into();
        style.visuals.widgets.hovered.corner_radius = 0.into();
        style.visuals.widgets.active.corner_radius = 0.into();
        ctx.set_style(style);
        let catalog = serde_json::from_str(include_str!("../assets/model-ui.json"))
            .expect("bundled model catalog");
        let mut app = Self {
            #[cfg(not(test))]
            dialogs: Box::new(NativeDialogs),
            #[cfg(test)]
            dialogs: Box::new(FakeDialogs::default()),
            document: None,
            catalog,
            artwork,
            textures: HashMap::new(),
            missing_textures: HashSet::new(),
            error: None,
            status: "Edit preset files offline. Your guitar is unchanged.".into(),
            tab: Tab::Body,
            help: false,
            pending: None,
            permit_close: false,
            drag_origin: None,
            theme_polled: Instant::now() - Duration::from_secs(2),
            theme_text: String::new(),
            screenshot: None,
            frame_count: 0,
            recording_open: false,
            recorder: recording::Panel::new(),
            pod_open: false,
            pod_library: Library::default(),
            pod_bundle: None,
            pod_selected: None,
            pod_cab_pending: None,
            pod_delay_model_pending: None,
            pod_mod_model_pending: None,
            pod_harmony_pending: None,
            pod_delay_pending: None,
            pod_mod_pending: None,
            pod_name: String::new(),
            pod_message: String::new(),
            #[cfg(test)]
            bank_rows: HashMap::new(),
            #[cfg(test)]
            pending_buttons: HashMap::new(),
            #[cfg(test)]
            pickup_slider: None,
            #[cfg(test)]
            recording_buttons: HashMap::new(),
            #[cfg(test)]
            recording_visible: String::new(),
            #[cfg(test)]
            recording_capture_attempts: 0,
            #[cfg(test)]
            pod_buttons: HashMap::new(),
            #[cfg(test)]
            pod_rows: HashMap::new(),
            #[cfg(test)]
            pod_cab_combo: None,
            #[cfg(test)]
            pod_cab_options: HashMap::new(),
            #[cfg(test)]
            pod_drive_slider: None,
            #[cfg(test)]
            pod_stomp_sliders: HashMap::new(),
            #[cfg(test)]
            pod_whammy_knob_sliders: HashMap::new(),
            #[cfg(test)]
            pod_eq_frequency_sliders: HashMap::new(),
            #[cfg(test)]
            pod_eq_gain_sliders: HashMap::new(),
            #[cfg(test)]
            pod_model_enable_checks: HashMap::new(),
            #[cfg(test)]
            pod_model_enable_menu: None,
            #[cfg(test)]
            pod_mod_sliders: HashMap::new(),
            #[cfg(test)]
            pod_reverb_sliders: HashMap::new(),
            #[cfg(test)]
            pod_delay_sliders: HashMap::new(),
            #[cfg(test)]
            pod_delay_note_options: HashMap::new(),
            #[cfg(test)]
            pod_delay_model_combo: None,
            #[cfg(test)]
            pod_delay_model_options: HashMap::new(),
            #[cfg(test)]
            pod_mod_model_combo: None,
            #[cfg(test)]
            pod_mod_model_options: HashMap::new(),
            #[cfg(test)]
            pod_mod_note_options: HashMap::new(),
            #[cfg(test)]
            pod_delay_time_slider: None,
            #[cfg(test)]
            pod_mod_speed_slider: None,
            #[cfg(test)]
            pod_delay_heads_choices: HashMap::new(),
            #[cfg(test)]
            pod_delay_bits_choices: HashMap::new(),
            #[cfg(test)]
            pod_stomp_wave_choices: HashMap::new(),
            #[cfg(test)]
            pod_harmony1_choices: HashMap::new(),
            #[cfg(test)]
            pod_harmony2_choices: HashMap::new(),
            #[cfg(test)]
            pod_visible: String::new(),
            read_receiver: None,
            read_worker: None,
        };
        if let Some(preset) = preset {
            app.load(&preset);
        }
        app
    }

    fn load(&mut self, path: &Path) {
        if self.is_reading() {
            return;
        }
        match EditorDocument::open(path) {
            Ok(document) => {
                self.document = Some(document);
                self.error = None;
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn dirty(&self) -> bool {
        // Guitar open and read prompts use this. POD tones use the quit guard.
        self.document.as_ref().is_some_and(EditorDocument::is_dirty)
    }

    fn pod_dirty(&self) -> bool {
        self.pod_library.any_dirty()
            || self
                .pod_bundle
                .as_ref()
                .is_some_and(pod_preset::PodBundleLibrary::is_dirty)
    }

    fn is_reading(&self) -> bool {
        self.read_receiver.is_some()
    }

    fn cancel_read(&mut self) {
        if let Some((cancel, _)) = &self.read_worker {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.status = "Cancelling guitar read…".into();
        }
    }

    fn undo(&mut self) {
        if !self.is_reading()
            && self.pending.is_none()
            && let Some(document) = &mut self.document
        {
            document.undo();
        }
    }

    fn redo(&mut self) {
        if !self.is_reading()
            && self.pending.is_none()
            && let Some(document) = &mut self.document
        {
            document.redo();
        }
    }

    fn start_read(&mut self) {
        // The user has locked all physical-device access during offline validation.
        if LIVE_ACCESS_LOCKED {
            self.status = "Live access is locked while offline validation is in progress.".into();
            return;
        }
        if self.is_reading() {
            return;
        }
        if self.dirty() {
            self.pending = Some(Pending::ReadCurrent);
            return;
        }
        self.launch_read();
    }

    fn launch_read(&mut self) {
        if LIVE_ACCESS_LOCKED {
            self.status = "Live access is locked while offline validation is in progress.".into();
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker = std::thread::spawn(move || {
            let _ = sender.send(variax_workbench::usb::read_current(&worker_cancel));
        });
        self.read_worker = Some((cancel, worker));
        self.read_receiver = Some(receiver);
        self.error = None;
        self.status = "Reading current model settings from the guitar…".into();
    }

    fn poll_read(&mut self) {
        let result = self
            .read_receiver
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Guitar reader stopped unexpectedly".into()))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.read_receiver = None;
            if let Some((_, worker)) = self.read_worker.take() {
                let _ = worker.join();
            }
            match result.and_then(|current| {
                let version = current.identity.version_string();
                let preset = PresetFile::from_metadata(
                    &current.metadata,
                    current.identity.product_id(),
                    current.identity.version.unwrap_or(0),
                )?;
                Ok((preset, version))
            }) {
                Ok((preset, version)) => {
                    self.document = Some(EditorDocument::from_preset(
                        preset,
                        PathBuf::from("Variax current model"),
                    ));
                    self.status = format!(
                        "Read current model settings from Variax, firmware {version}. Save a copy to keep them."
                    );
                }
                Err(error) => {
                    self.error = Some(error);
                    self.status =
                        "Guitar read did not complete. Your current document is retained.".into();
                }
            }
        }
    }

    fn artwork_dialog(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(path) = self.dialogs.artwork() {
            self.artwork = path;
            self.textures.clear();
            self.missing_textures.clear();
            self.error = None;
        }
    }

    fn open_dialog(&mut self) {
        if self.is_reading() {
            return;
        }
        if self.pending.is_some() {
            return;
        }
        if let Some(path) = self.dialogs.open() {
            if self.dirty() {
                self.pending = Some(Pending::Open(path));
            } else {
                self.load(&path);
            }
        }
    }

    fn save_dialog(&mut self) -> bool {
        if self.is_reading() {
            return false;
        }
        let Some(document) = &mut self.document else {
            return false;
        };
        let name: String = document
            .preset()
            .name()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let extension = document.extension();
        let name = format!("{name} - edited.{extension}");
        if let Some(path) = self.dialogs.save(&name, extension) {
            match document.save_copy(&path) {
                Ok(()) => {
                    self.error = None;
                    self.status = format!("Saved copy: {}", path.display());
                    return true;
                }
                Err(error) => self.error = Some(error),
            }
        }
        false
    }

    fn continue_pending(&mut self, ctx: &egui::Context) {
        if self.is_reading() {
            return;
        }
        match self.pending.take() {
            Some(Pending::Open(path)) => self.load(&path),
            Some(Pending::ReadCurrent) => self.launch_read(),
            Some(Pending::Quit | Pending::QuitPod | Pending::QuitRecording) => {
                // Guitar save or discard must not close while a POD tone is unsaved.
                if self.pod_dirty() {
                    self.pending = Some(Pending::QuitPod);
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                } else if self.recorder.has_unsaved_take() {
                    self.pending = Some(Pending::QuitRecording);
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                } else {
                    self.permit_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            None => {}
        }
    }

    fn bank_browser(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        self.bank_rows.clear();
        let Some(document) = &self.document else {
            return;
        };
        let Some(banks) = document.banks() else {
            return;
        };
        let selection = document.selection();
        let labels: Vec<_> = banks
            .iter()
            .map(|b| {
                (
                    b.name.clone(),
                    b.presets()
                        .iter()
                        .map(|p| p.name().to_owned())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut next = None;
        egui::SidePanel::left("bank-browser")
            .default_width(210.)
            .show(ctx, |ui| {
                ui.heading("Banks and presets");
                ui.add_enabled_ui(!self.is_reading() && self.pending.is_none(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (bank, (name, presets)) in labels.iter().enumerate() {
                            egui::CollapsingHeader::new(format!("{} · {}", bank + 1, name))
                                .id_salt(("bank", bank))
                                .default_open(true)
                                .show(ui, |ui| {
                                    if presets.is_empty() {
                                        ui.weak("Empty bank");
                                    }
                                    for (preset, name) in presets.iter().enumerate() {
                                        let response = ui.selectable_label(
                                            selection == Some((bank, preset)),
                                            format!("{} · {}", preset + 1, name),
                                        );
                                        #[cfg(test)]
                                        self.bank_rows.insert((bank, preset), response.rect);
                                        if response.clicked() {
                                            next = Some((bank, preset));
                                        }
                                    }
                                });
                        }
                    });
                });
            });
        if let Some((bank, preset)) = next
            && let Some(document) = &mut self.document
        {
            match document.select(bank, preset) {
                Ok(()) => {
                    self.drag_origin = None;
                    self.error = None;
                    self.status = format!(
                        "Bank {}, preset {}. Edits stay in this bank copy.",
                        bank + 1,
                        preset + 1
                    );
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn change(&mut self, change: impl FnOnce(&mut PresetFile) -> Result<(), String>) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document
            && let Err(error) = document.apply(change)
        {
            self.error = Some(error);
        }
    }

    fn can_edit(&self, slot: u32, id: Option<u32>) -> bool {
        self.document
            .as_ref()
            .is_some_and(|d| d.preset().edit_availability(slot, id).is_ok())
    }

    fn model_id(&self, slot: u32) -> Option<u32> {
        self.document.as_ref()?.preset().model(slot).map(|m| m.id)
    }
    fn model_name(&self, slot: u32) -> String {
        self.model_id(slot)
            .and_then(|id| self.catalog.iter().find(|a| a.id == id))
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown model".into())
    }
    fn float(&self, slot: u32, id: u32) -> Option<f32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 1).then(|| f32::from_bits(p.bits))
    }
    fn integer(&self, slot: u32, id: u32) -> Option<i32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 0).then_some(p.bits as i32)
    }

    fn float_control(&mut self, ui: &mut egui::Ui, slot: u32, id: u32, label: &str, suffix: &str) {
        if let Some(mut value) = self.float(slot, id) {
            let Some(variax_workbench::edit::Domain::Float { min, max, .. }) =
                variax_workbench::edit::domain(slot, id)
            else {
                return;
            };
            if !value.is_finite() {
                ui.horizontal(|ui| {
                    ui.label(format!("{label}: {value} (stored)"));
                    if ui
                        .add_enabled(
                            self.can_edit(slot, Some(id)),
                            egui::Button::new("Reset to minimum"),
                        )
                        .clicked()
                    {
                        self.change(|p| p.set_parameter(slot, id, 1, min.to_bits()));
                    }
                });
                return;
            }
            let response = ui.add_enabled(
                self.can_edit(slot, Some(id)),
                egui::Slider::new(&mut value, min..=max)
                    .clamping(egui::SliderClamping::Edits)
                    .text(label)
                    .suffix(suffix),
            );
            #[cfg(test)]
            if slot == PICKUP1 && id == 0x200000 {
                self.pickup_slider = Some(response.rect);
            }
            if response.drag_started()
                && let Some(document) = &mut self.document
            {
                document.begin_gesture();
            }
            if response.changed() {
                self.change(|p| p.set_parameter(slot, id, 1, value.to_bits()));
            }
            if response.drag_stopped()
                && let Some(document) = &mut self.document
            {
                document.end_gesture();
            }
        }
    }
    fn float_drag_control(&mut self, ui: &mut egui::Ui, slot: u32, id: u32) {
        let Some(mut value) = self.float(slot, id) else {
            ui.label("—");
            return;
        };
        let Some(variax_workbench::edit::Domain::Float { min, max, .. }) =
            variax_workbench::edit::domain(slot, id)
        else {
            return;
        };
        if !value.is_finite() {
            ui.label(format!("{value} (stored)"));
            return;
        }
        if ui
            .add_enabled(
                self.can_edit(slot, Some(id)),
                egui::DragValue::new(&mut value)
                    .range(min..=max)
                    .clamp_existing_to_range(false)
                    .speed(0.01),
            )
            .changed()
        {
            self.change(|p| p.set_parameter(slot, id, 1, value.to_bits()));
        }
    }

    fn boolean_control(&mut self, ui: &mut egui::Ui, slot: u32, id: u32, label: &str) {
        if let Some(value) = self.integer(slot, id) {
            let mut checked = value != 0;
            if ui
                .add_enabled(
                    self.can_edit(slot, Some(id)),
                    egui::Checkbox::new(&mut checked, label),
                )
                .changed()
            {
                self.change(|p| p.set_parameter(slot, id, 0, u32::from(checked)));
            }
        }
    }

    fn float_choices(&mut self, ui: &mut egui::Ui, id: u32, label: &str, choices: &[(f32, &str)]) {
        let Some(value) = self.float(WIRING, id) else {
            return;
        };
        let mut selected = value.to_bits();
        let text = choices
            .iter()
            .find(|(candidate, _)| {
                // Values read from the guitar have only the high 24 IEEE bits.
                (candidate.to_bits() & 0xffff_ff00) == (value.to_bits() & 0xffff_ff00)
            })
            .map(|(_, text)| (*text).to_owned())
            .unwrap_or_else(|| format!("{value:.6} (stored)"));
        ui.horizontal(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt(("electrical", id))
                .selected_text(text)
                .width(150.0)
                .show_ui(ui, |ui| {
                    for &(candidate, text) in choices {
                        ui.selectable_value(&mut selected, candidate.to_bits(), text);
                    }
                });
        });
        if selected != value.to_bits() {
            self.change(|p| p.set_parameter(WIRING, id, 1, selected));
        }
    }

    fn taper_control(&mut self, ui: &mut egui::Ui) {
        let Some(mut value) = self.integer(WIRING, 0x200001) else {
            return;
        };
        let original = value;
        ui.horizontal(|ui| {
            ui.label("Volume taper");
            egui::ComboBox::from_id_salt("volume-taper")
                .selected_text(match value {
                    0 => "Linear",
                    1 => "Audio",
                    _ => "Unknown (stored)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut value, 0, "Linear");
                    ui.selectable_value(&mut value, 1, "Audio");
                });
        });
        if value != original {
            self.change(|p| p.set_parameter(WIRING, 0x200001, 0, value as u32));
        }
    }
    fn enabled_control(&mut self, ui: &mut egui::Ui, slot: u32, label: &str) {
        if let Some(model) = self.document.as_ref().and_then(|d| d.preset().model(slot)) {
            let mut enabled = model.enabled;
            if ui
                .add_enabled(
                    self.can_edit(slot, None),
                    egui::Checkbox::new(&mut enabled, label),
                )
                .changed()
            {
                self.change(|p| p.set_enabled(slot, enabled));
            }
        }
    }
    fn model_control(&mut self, ui: &mut egui::Ui, slot: u32, kind: &str) {
        let Some(mut id) = self.model_id(slot) else {
            return;
        };
        let previous = id;
        egui::ComboBox::from_id_salt(("model", slot))
            .selected_text(self.model_name(slot))
            .width(280.0)
            .show_ui(ui, |ui| {
                for model in self.catalog.iter().filter(|a| a.kind == kind) {
                    ui.selectable_value(&mut id, model.id, &model.name);
                }
            });
        if id != previous {
            self.change(|p| p.select_model(slot, id));
        }
    }

    fn texture(&mut self, ctx: &egui::Context, relative: &str) -> Option<TextureHandle> {
        if let Some(texture) = self.textures.get(relative) {
            return Some(texture.clone());
        }
        if self.missing_textures.contains(relative) {
            return None;
        }
        let path = self.artwork.join(relative);
        match image::open(&path) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                let texture = ctx.load_texture(relative, pixels, egui::TextureOptions::LINEAR);
                self.textures.insert(relative.to_owned(), texture.clone());
                Some(texture)
            }
            Err(_) => {
                self.missing_textures.insert(relative.to_owned());
                None
            }
        }
    }

    fn guitar(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(
            ui.available_width().min(560.0),
            ui.available_width().min(560.0) * 412.0 / 450.0,
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let scale = size.x / 450.0;
        let body = self
            .model_id(BODY)
            .and_then(|id| self.catalog.iter().find(|a| a.id == id))
            .map(|a| a.image.clone());
        if let Some(body) = body
            && let Some(texture) = self.texture(ui.ctx(), &body)
        {
            let photo = Rect::from_min_size(rect.min, Vec2::new(426.0, 412.0) * scale);
            ui.painter().image(
                texture.id(),
                photo,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        for slot in [PICKUP2, PICKUP1] {
            let Some(model) = self.document.as_ref().and_then(|d| d.preset().model(slot)) else {
                continue;
            };
            let enabled = model.enabled;
            let artwork = self
                .catalog
                .iter()
                .find(|a| a.id == model.id)
                .map(|a| a.image.clone())
                .unwrap_or_else(|| "pickups/unknown.png".into());
            let position = self.float(slot, 0x200000).unwrap_or(2.0);
            let angle = self.float(slot, 0x200001).unwrap_or(0.0).to_radians();
            if !position.is_finite() || !angle.is_finite() {
                continue;
            }
            let center = rect.min
                + Vec2::new(
                    68.0 + 318.0 * ((position.clamp(0.1, 9.5) - 0.1) / 9.4),
                    215.0,
                ) * scale;
            if let Some(texture) = self.texture(ui.ctx(), &artwork) {
                let dimension = texture.size_vec2() * scale;
                let rotation = egui::emath::Rot2::from_angle(angle);
                let corners = [
                    Vec2::new(-0.5, -0.5),
                    Vec2::new(0.5, -0.5),
                    Vec2::new(0.5, 0.5),
                    Vec2::new(-0.5, 0.5),
                ];
                let uvs = [
                    Pos2::new(0.0, 0.0),
                    Pos2::new(1.0, 0.0),
                    Pos2::new(1.0, 1.0),
                    Pos2::new(0.0, 1.0),
                ];
                let mut mesh = egui::Mesh::with_texture(texture.id());
                for (corner, uv) in corners.into_iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: center + rotation * (corner * dimension),
                        uv,
                        color: Color32::from_white_alpha(if enabled { 255 } else { 102 }),
                    });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                ui.painter().add(egui::Shape::mesh(mesh));
                let response = ui
                    .interact(
                        Rect::from_center_size(center, dimension + Vec2::splat(12.0)),
                        egui::Id::new(("pickup-drag", slot)),
                        if self.can_edit(slot, Some(0x200000)) {
                            Sense::drag()
                        } else {
                            Sense::hover()
                        },
                    )
                    .on_hover_text(
                        "Drag horizontally to move this pickup. Adjust rotation in Pickups.",
                    );
                if response.drag_started() {
                    self.drag_origin = Some((slot, position));
                    if let Some(document) = &mut self.document {
                        document.begin_gesture();
                    }
                }
                if response.dragged()
                    && let Some((active, original)) = self.drag_origin
                    && active == slot
                {
                    let value =
                        (original + response.drag_delta().x / scale / 318.0 * 9.4).clamp(0.1, 9.5);
                    self.change(|p| p.set_parameter(slot, 0x200000, 1, value.to_bits()));
                }
                if response.drag_stopped() {
                    self.drag_origin = None;
                    if let Some(document) = &mut self.document {
                        document.end_gesture();
                    }
                }
            }
        }
        ui.label(
            egui::RichText::new(self.model_name(BODY))
                .size(23.0)
                .strong(),
        );
        if self.textures.is_empty() {
            ui.weak("Artwork is optional. Choose an extracted editor image folder to show the original graphics. All parameter controls remain available.");
        } else {
            ui.weak("Original Workbench artwork · drag pickups to position them");
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, title) in [
                (Tab::Body, "Body"),
                (Tab::Pickups, "Pickups"),
                (Tab::Tuning, "Tuning"),
                (Tab::Controls, "Controls"),
            ] {
                ui.selectable_value(&mut self.tab, tab, title);
            }
        });
        ui.separator();
        let slot = match self.tab {
            Tab::Body => BODY,
            Tab::Pickups => PICKUP1,
            Tab::Tuning => TUNING,
            Tab::Controls => variax_workbench::edit::SYSTEM,
        };
        let availability = self
            .document
            .as_ref()
            .map(|d| d.preset().edit_availability(slot, None));
        if let Some(Err(reason)) = &availability {
            ui.label(reason);
        }
        ui.add_enabled_ui(availability.is_some_and(|a| a.is_ok()), |ui| { match self.tab {
            Tab::Body => {
                ui.heading("Guitar body");
                ui.label("Choose the modeled instrument body.");
                self.model_control(ui, BODY, "body");
                ui.add_space(16.0);
                ui.heading("Pickup combination");
                ui.label(format!("Pickup 1: {}", self.model_name(PICKUP1)));
                ui.label(format!("Pickup 2: {}", self.model_name(PICKUP2)));
                ui.add_space(16.0);
                ui.label("Edits stay in your preset copy. Sending changes to the guitar is not available yet.");
            }
            Tab::Pickups => {
                for (slot, title) in [(PICKUP1, "Pickup 1"), (PICKUP2, "Pickup 2")] {
                    ui.heading(title);
                    self.enabled_control(ui, slot, "Enabled");
                    self.model_control(ui, slot, "pickup");
                    self.float_control(ui, slot, 0x200000, "Position", " in");
                    self.float_control(ui, slot, 0x200001, "Angle", "°");
                    self.float_control(ui, slot, 0x200003, "Level", " dB");
                    ui.add_space(12.0);
                }
                self.boolean_control(ui, PICKUP2, 0x200002, "Reverse pickup 2 phase");
                self.boolean_control(ui, PICKUP2, 0x200004, "Wire pickups in series");
            }
            Tab::Tuning => {
                ui.heading("Alternate tuning");
                self.enabled_control(ui, TUNING, "Enable alternate tuning");
                ui.label("Strings are shown from low E to high E.");
                egui::Grid::new("tuning").striped(true).show(ui, |ui| {
                    ui.strong("String");
                    ui.strong("Semitones");
                    ui.strong("Detune (cents)");
                    ui.strong("Mix");
                    ui.end_row();
                    for (i, label) in [
                        "6 · Low E",
                        "5 · A",
                        "4 · D",
                        "3 · G",
                        "2 · B",
                        "1 · High E",
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.label(label);
                        if let Some(mut value) = self.integer(TUNING, 0x200000 + i as u32) {
                            if ui
                                .add(egui::DragValue::new(&mut value).range(-12..=12).clamp_existing_to_range(false))
                                .changed()
                            {
                                self.change(|p| {
                                    p.set_parameter(TUNING, 0x200000 + i as u32, 0, value as u32)
                                });
                            }
                        } else {
                            ui.label("—");
                        }
                        self.float_drag_control(ui, TUNING, 0x200006 + i as u32);
                        self.float_drag_control(ui, TUNING, 0x20000c + i as u32);
                        ui.end_row();
                    }
                });
                self.float_control(ui, TUNING, 0x200012, "String spread", "");
            }
            Tab::Controls => {
                let availability = self.document.as_ref().unwrap().preset().edit_availability(WIRING, None);
                if let Err(reason) = &availability { ui.label(reason); }
                ui.add_enabled_ui(availability.is_ok(), |ui| {
                ui.heading("Volume and tone");
                let resistances = [
                    (25.0, "25 kΩ"),
                    (50.0, "50 kΩ"),
                    (100.0, "100 kΩ"),
                    (250.0, "250 kΩ"),
                    (470.0, "470 kΩ"),
                    (500.0, "500 kΩ"),
                    (1000.0, "1 MΩ"),
                ];
                self.float_choices(ui, 0x200000, "Volume resistance", &resistances);
                self.taper_control(ui);
                self.float_choices(ui, 0x200002, "Tone resistance", &resistances);
                self.float_choices(
                    ui,
                    0x200003,
                    "Tone capacitance",
                    &[
                        (0.0011, "1.1 nF"),
                        (0.0022, "2.2 nF"),
                        (0.0047, "4.7 nF"),
                        (0.011, "11 nF"),
                        (0.022, "22 nF"),
                        (0.047, "47 nF"),
                        (0.1, "0.1 µF"),
                        (-0.047, "Filter"),
                    ],
                );
                self.float_control(ui, WIRING, 0x200004, "Tone position", "");
                self.boolean_control(ui, WIRING, 0x200005, "Save tone-knob position");
                });
                ui.add_space(12.0);
                self.float_control(ui, 0x02010000, 0x200000, "Output trim", " dB");
            }
        }
        });
    }

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
        self.render(ctx);
    }
}

impl Workbench {
    fn pending_dialog(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        self.pending_buttons.clear();
        if matches!(self.pending, Some(Pending::QuitRecording)) {
            self.recording_quit_dialog(ctx);
            return;
        }
        if matches!(self.pending, Some(Pending::QuitPod)) {
            self.pod_quit_dialog(ctx);
            return;
        }
        if self.pending.is_some() {
            let mut discard = false;
            let mut save_continue = false;
            let mut cancel = false;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Save a copy to keep your changes before continuing.");
                    ui.horizontal(|ui| {
                        let response = ui.button("Save copy and continue…");
                        #[cfg(test)]
                        self.pending_buttons.insert("save", response.rect);
                        if response.clicked() {
                            save_continue = true;
                        }
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", response.rect);
                        if response.clicked() {
                            cancel = true;
                        }
                        let response = ui.button("Discard changes");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", response.rect);
                        if response.clicked() {
                            discard = true;
                        }
                    });
                });
            if cancel {
                self.pending = None;
            }
            if save_continue {
                discard = self.save_dialog();
            }
            if discard {
                self.continue_pending(ctx);
            }
        }
    }

    fn request_close(&mut self, ctx: &egui::Context) {
        if self.is_reading() {
            self.cancel_read();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.pending.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.dirty() && !self.permit_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        } else if self.pod_dirty() && !self.permit_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::QuitPod);
        } else if self.recorder.has_unsaved_take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::QuitRecording);
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn recording_quit_dialog(&mut self, ctx: &egui::Context) {
        if !self.recorder.has_unsaved_take() {
            self.continue_pending(ctx);
            return;
        }
        let native = self.recorder.take_can_save_aiff();
        let can_save = self.recorder.take_can_save_wav() || native;
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved offline test take")
            .default_width(560.0)
            .max_width(640.0)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This in-memory test take has not been saved.");
                if native {
                    ui.label("Exact-clock AIFF preserves 78125/2. Other software may round this rate; verify imports.");
                }
                ui.label(self.recorder.message());
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(can_save, egui::Button::new(if native { "Stop and save AIFF copy…" } else { "Stop and save WAV copy…" }));
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this take");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save {
            if self.recorder.phase() == recording::Phase::Recording
                && let Err(error) = self.recorder.stop()
            {
                self.recorder.note(error.to_string());
                return;
            }
            if self.save_recording() {
                self.continue_pending(ctx);
            }
        } else if discard {
            match self.recorder.cancel() {
                Ok(()) => self.continue_pending(ctx),
                Err(error) => self.recorder.note(error.to_string()),
            }
        }
    }

    fn pod_quit_dialog(&mut self, ctx: &egui::Context) {
        if self
            .pod_bundle
            .as_ref()
            .is_some_and(|bundle| bundle.is_dirty())
        {
            self.pod_bundle_quit_dialog(ctx);
            return;
        }
        let Some(index) = self.pod_library.first_dirty() else {
            self.continue_pending(ctx);
            return;
        };
        let Some(name) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.name().to_owned())
        else {
            self.pending = None;
            return;
        };
        let remaining = self.pod_library.dirty_count();
        let still = if remaining == 1 {
            "This is the only unsaved tone.".to_owned()
        } else {
            format!(
                "{remaining} tones are unsaved. Saving or discarding this one leaves the others unsaved."
            )
        };
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved PODxt Live tone")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "\"{name}\" has an applied name edit that has not been saved. {still}"
                ));
                ui.horizontal(|ui| {
                    let response = ui.button("Save copy and continue…");
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    if response.clicked() {
                        save_continue = true;
                    }
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    if response.clicked() {
                        cancel = true;
                    }
                    let response = ui.button("Discard this tone");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    if response.clicked() {
                        discard = true;
                    }
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            if self.save_next_dirty_pod() && !self.pod_library.any_dirty() {
                self.continue_pending(ctx);
            }
        } else if discard && self.discard_next_dirty_pod() && !self.pod_library.any_dirty() {
            self.continue_pending(ctx);
        }
    }

    fn pod_bundle_quit_dialog(&mut self, ctx: &egui::Context) {
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved POD bank or collection")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This bank/collection has offline edits that have not been saved.");
                ui.horizontal(|ui| {
                    let response = ui.button("Save bank copy and continue…");
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard bank edits");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            let index = self.pod_library.first_dirty().unwrap_or(0);
            if self.write_pod_copy(index) && !self.pod_library.any_dirty() {
                self.continue_pending(ctx);
            }
        } else if discard {
            let result = self
                .pod_bundle
                .as_mut()
                .map_or(Ok(()), |bundle| bundle.revert());
            match result.and_then(|()| self.refresh_pod_library_from_bundle()) {
                Ok(()) => {
                    self.refresh_selected_pod_name();
                    self.pod_message = "Discarded offline bank edits. Nothing was sent.".into();
                    self.continue_pending(ctx);
                }
                Err(error) => self.pod_message = error.detail(),
            }
        }
    }

    fn save_next_dirty_pod(&mut self) -> bool {
        let Some(index) = self.pod_library.first_dirty() else {
            return false;
        };
        self.write_pod_copy(index)
    }

    fn discard_next_dirty_pod(&mut self) -> bool {
        if let Some(bundle) = &mut self.pod_bundle {
            return match bundle
                .revert()
                .and_then(|()| self.refresh_pod_library_from_bundle())
            {
                Ok(()) => {
                    self.refresh_selected_pod_name();
                    self.pod_message = "Discarded unsaved bank edits. Nothing was sent.".into();
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            };
        }
        let Some(index) = self.pod_library.first_dirty() else {
            return false;
        };
        match self.pod_library.revert_unsaved(index) {
            Ok(()) => {
                self.refresh_selected_pod_name();
                self.pod_message =
                    "Discarded the unsaved name edit for one tone. Nothing was sent.".into();
                true
            }
            Err(error) => {
                self.pod_message = error.detail();
                false
            }
        }
    }

    fn refresh_selected_pod_name(&mut self) {
        self.pod_cab_pending = None;
        self.pod_delay_model_pending = None;
        self.pod_mod_model_pending = None;
        self.pod_harmony_pending = None;
        self.pod_delay_pending = None;
        self.pod_mod_pending = None;
        match (self.pod_selected, self.pod_library.len()) {
            (Some(_), 0) => {
                self.pod_selected = None;
                self.pod_name.clear();
            }
            (Some(index), len) if index >= len => {
                let selected = len - 1;
                self.pod_selected = Some(selected);
                self.pod_name = self
                    .pod_library
                    .tone(selected)
                    .map_or_else(String::new, |tone| tone.name().to_owned());
            }
            (Some(index), _) => {
                self.pod_name = self
                    .pod_library
                    .tone(index)
                    .map_or_else(String::new, |tone| tone.name().to_owned());
            }
            (None, _) => self.pod_name.clear(),
        }
    }

    fn render(&mut self, ctx: &egui::Context) {
        self.refresh_theme(ctx);
        self.poll_read();
        if self.read_receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.frame_count += 1;
        if self.screenshot.is_some() && self.frame_count <= 5 {
            ctx.request_repaint();
            if self.frame_count == 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image: capture, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let pixels: Vec<u8> = capture
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Err(error) = save_screenshot(
                    &path,
                    &pixels,
                    capture.size[0] as u32,
                    capture.size[1] as u32,
                ) {
                    self.error = Some(format!("Screenshot: {error}"));
                } else {
                    self.request_close(ctx);
                }
            }
        }
        let (open, save, undo, redo, quit, help) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::O) && i.modifiers.command,
                i.key_pressed(egui::Key::S) && i.modifiers.command,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift,
                i.key_pressed(egui::Key::Q) && !ctx.wants_keyboard_input(),
                i.key_pressed(egui::Key::Questionmark) && !ctx.wants_keyboard_input(),
            )
        });
        if open {
            self.open_dialog();
        }
        if save && self.pending.is_none() {
            self.save_dialog();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if help {
            self.help = !self.help;
        }
        if quit || ctx.input(|i| i.viewport().close_requested()) {
            self.request_close(ctx);
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("Variax Workbench");
                ui.weak("Offline validation");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("? Shortcuts").clicked() {
                        self.help = true;
                    }
                    if ui.add_enabled(!self.is_reading() && self.pending.is_none(), egui::Button::new("Save copy…")).clicked() {
                        self.save_dialog();
                    }
                    if ui
                        .add_enabled(
                            false,
                            egui::Button::new("Live access locked"),
                        )
                        .on_hover_text("Physical device access is disabled while offline validation is in progress.")
                        .clicked()
                    {
                        self.start_read();
                    }
                    if ui.add_enabled(!self.is_reading() && self.pending.is_none(), egui::Button::new("Open…")).clicked() {
                        self.open_dialog();
                    }
                    let response = ui.button("Test recording…");
                    #[cfg(test)]
                    self.recording_buttons.insert("open-recording", response.rect);
                    if response.clicked() {
                        self.recording_open = true;
                    }
                    let response = ui.button("POD tones…");
                    #[cfg(test)]
                    self.pod_buttons.insert("open-pod", response.rect);
                    if response.clicked() {
                        self.pod_open = true;
                    }
                });
            });
            ui.add_space(8.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if self.is_reading() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading model settings…");
                    if ui.button("Cancel read").clicked() {
                        self.cancel_read();
                    }
                });
            }
            ui.vertical(|ui| {
                ui.weak(&self.status);
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
            });
        });
        self.bank_browser(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(document) = &self.document else {
                ui.heading("Open a Variax preset, bank or collection to begin"); return;
            };
            let mut name = document.preset().name().to_owned();
            let editable = matches!(document.preset().device_id(), 0x70000 | 0x70004);
            let dirty = document.is_dirty();
            let can_edit = editable && !self.is_reading() && self.pending.is_none();
            let can_undo = document.can_undo(); let can_redo = document.can_redo();
            ui.horizontal(|ui| {
                ui.label("Preset");
                if ui.add_enabled(can_edit, egui::TextEdit::singleline(&mut name)).changed() { self.change(|p| p.rename(&name)); }
                if dirty { ui.label(egui::RichText::new("Unsaved changes").color(Color32::from_rgb(255,200,90))); }
                if ui.add_enabled(can_undo && can_edit, egui::Button::new("Undo")).clicked() { self.undo(); }
                if ui.add_enabled(can_redo && can_edit, egui::Button::new("Redo")).clicked() { self.redo(); }
            });
            ui.add_space(16.0);
            if !editable { ui.label("This preset can be inspected. Editing currently supports legacy electric Variax models."); }
            ui.columns(2, |columns| {
                egui::Frame::group(columns[0].style()).show(&mut columns[0], |ui| {
                    ui.add_enabled_ui(can_edit, |ui| self.guitar(ui));
                    ui.add_space(12.0);
                    if ui.add_enabled(!self.is_reading() && self.pending.is_none(), egui::Button::new("Artwork folder…").small()).clicked() {
                        self.artwork_dialog();
                    }
                });
                egui::Frame::group(columns[1].style()).show(&mut columns[1], |ui| {
                    ui.add_enabled_ui(can_edit, |ui| { egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui)); });
                });
            });
        });
        if self.help {
            egui::Window::new("Keyboard shortcuts")
                .open(&mut self.help)
                .collapsible(false)
                .show(ctx, |ui| {
                    for (key, action) in [
                        ("Ctrl+O", "Open preset"),
                        ("Ctrl+S", "Save a new copy"),
                        ("Ctrl+Z", "Undo"),
                        ("Ctrl+Shift+Z", "Redo"),
                        ("Q", "Quit"),
                        ("?", "Show shortcuts"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.monospace(key);
                            ui.label(action);
                        });
                    }
                });
        }
        self.pending_dialog(ctx);
        self.recording_window(ctx);
        self.pod_window(ctx);
    }

    fn recording_window(&mut self, ctx: &egui::Context) {
        if !self.recording_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Offline test recording")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 64.0))
            .show(ctx, |ui| {
                if self.pending.is_some() {
                    ui.disable();
                }
                ui.label(recording::OFFLINE_BANNER);
                ui.label(recording::DEVICE_CAPTURE_LOCKED);
                ui.label(recording::NO_AUTO_START);
                ui.label(self.recorder.phase_label());
                ui.label(self.recorder.meter_line());
                ui.label(self.recorder.guide_line());
                ui.label(self.recorder.message());
                ui.separator();
                for (route, label) in recording::ROUTE_CHOICES {
                    if self.record_choice(
                        ui,
                        route_button_id(route),
                        self.recorder.route() == Some(route),
                        label,
                    ) && let Err(error) = self.recorder.select_route(route)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                for (clock, label) in recording::CLOCK_CHOICES {
                    if self.record_choice(
                        ui,
                        clock_button_id(clock),
                        self.recorder.clock() == clock,
                        label,
                    ) && let Err(error) = self.recorder.select_clock(clock)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                if self.record_choice(
                    ui,
                    "input-synthetic",
                    self.recorder.input() == recording::InputSelection::Synthetic,
                    "Synthetic generator",
                ) && let Err(error) = self
                    .recorder
                    .select_input(recording::InputSelection::Synthetic)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_choice(
                    ui,
                    "input-file",
                    self.recorder.input() == recording::InputSelection::File,
                    "WAV file",
                ) && let Err(error) = self.recorder.select_input(recording::InputSelection::File)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_button(ui, "load-wav", "Load test WAV…") {
                    self.load_recording_wav();
                }
                ui.separator();
                if self.record_button(ui, "start", "Start") {
                    self.arm_recording();
                }
                if self.record_button(ui, "feed", "Feed chunk")
                    && let Err(error) = self.recorder.feed()
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_button(ui, "stop", "Stop")
                    && let Err(error) = self.recorder.stop()
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_button(ui, "cancel", "Cancel")
                    && let Err(error) = self.recorder.cancel()
                {
                    self.recorder.note(error.to_string());
                }
                let label = if self.recorder.take_can_save_aiff() {
                    "Save exact-clock AIFF…"
                } else {
                    "Save WAV copy…"
                };
                if self.record_button(ui, "save", label) {
                    self.save_recording();
                }
                #[cfg(test)]
                {
                    self.recording_visible = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        recording::OFFLINE_BANNER,
                        recording::DEVICE_CAPTURE_LOCKED,
                        recording::NO_AUTO_START,
                        self.recorder.phase_label(),
                        self.recorder.meter_line(),
                        self.recorder.guide_line(),
                    );
                }
            });
        self.recording_open = open;
    }

    fn pod_window(&mut self, ctx: &egui::Context) {
        if !self.pod_open {
            return;
        }
        #[cfg(test)]
        {
            self.pod_rows.clear();
            self.pod_cab_combo = None;
            self.pod_cab_options.clear();
            self.pod_drive_slider = None;
            self.pod_stomp_sliders.clear();
            self.pod_whammy_knob_sliders.clear();
            self.pod_eq_frequency_sliders.clear();
            self.pod_eq_gain_sliders.clear();
            self.pod_model_enable_checks.clear();
            self.pod_model_enable_menu = None;
            self.pod_mod_sliders.clear();
            self.pod_reverb_sliders.clear();
            self.pod_delay_sliders.clear();
            self.pod_delay_note_options.clear();
            self.pod_delay_time_slider = None;
            self.pod_mod_note_options.clear();
            self.pod_mod_speed_slider = None;
            self.pod_delay_heads_choices.clear();
            self.pod_delay_bits_choices.clear();
            self.pod_stomp_wave_choices.clear();
            self.pod_harmony1_choices.clear();
            self.pod_harmony2_choices.clear();
        }
        let mut open = true;
        egui::Window::new("Offline PODxt Live tones")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 64.0))
            .show(ctx, |ui| {
                #[cfg(test)]
                {
                    self.pod_drive_slider = None;
                }
                if self.pending.is_some() {
                    ui.disable();
                }
                ui.label(pod_preset::OFFLINE_BANNER);
                ui.label(pod_preset::NOT_GUITAR);
                ui.label(pod_preset::NOT_SENT);
                ui.label(pod_preset::PAYLOADS_NOT_DECODED);
                ui.label(&self.pod_message);
                ui.separator();
                let membership_selection = self.pod_selected.and_then(|index| {
                    self.pod_bundle.as_ref().and_then(|bundle| {
                        let (bank, preset) = bundle.entry_location(index)?;
                        let folder_editable = bundle.membership_editable(bank);
                        let fixed_bank_editable = bundle.fixed_bank_reorder_editable(bank);
                        (folder_editable || fixed_bank_editable).then_some((
                            bank,
                            preset,
                            bundle.preset_count(bank)?,
                            folder_editable,
                        ))
                    })
                });
                let mut membership_action = None;
                ui.horizontal(|ui| {
                    if self.pod_button(ui, "load-pod", "Load tone or bank…") {
                        self.load_pod_tone();
                    }
                    if self.pod_button(ui, "apply-pod-name", "Apply name") {
                        self.apply_pod_name();
                    }
                    if self.pod_button(ui, "save-pod", "Save copy…") {
                        self.save_pod_tone();
                    }
                    if let Some((bank, preset, count, folder_editable)) = membership_selection {
                        if folder_editable {
                            if self.pod_button(ui, "pod-duplicate-entry", "Duplicate") {
                                membership_action =
                                    Some((PodMembershipAction::Duplicate, bank, preset));
                            }
                            if self.pod_button(ui, "pod-remove-entry", "Remove") {
                                membership_action =
                                    Some((PodMembershipAction::Remove, bank, preset));
                            }
                        }
                        if self.pod_button_enabled(
                            ui,
                            "pod-move-entry-up",
                            "Move up",
                            preset > 0,
                        ) {
                            membership_action = Some((PodMembershipAction::MoveUp, bank, preset));
                        }
                        if self.pod_button_enabled(
                            ui,
                            "pod-move-entry-down",
                            "Move down",
                            preset + 1 < count,
                        ) {
                            membership_action =
                                Some((PodMembershipAction::MoveDown, bank, preset));
                        }
                        let slot_kind = if folder_editable { "Folder" } else { "Fixed bank" };
                        ui.label(format!("{slot_kind} slot {} of {count}", preset + 1));
                    }
                    if let Some(index) = self.pod_selected {
                        let _response = ui.menu_button("Model flags…", |ui| {
                            self.model_enable_menu(ui, index);
                        });
                        #[cfg(test)]
                        {
                            self.pod_model_enable_menu = Some(_response.response.rect);
                        }
                    }
                });
                if let Some((action, bank, preset)) = membership_action {
                    self.apply_pod_membership_action(action, bank, preset);
                }
                let rows: Vec<_> = (0..self.pod_library.len())
                    .filter_map(|index| {
                        let tone = self.pod_library.tone(index)?;
                        let bank = self
                            .pod_bundle
                            .as_ref()
                            .and_then(|bundle| {
                                bundle.entry_location(index).and_then(|(bank, preset)| {
                                bundle
                                    .bank_name(bank)
                                    .map(|name| format!("bank {name} preset {preset} · "))
                                })
                            })
                            .unwrap_or_default();
                        Some((
                            index,
                            format!(
                                "{bank}{index} {} {} amp {} stomp {} cab {}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.stomp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                            ),
                            tone.name().to_owned(),
                        ))
                    })
                    .collect();
                for (index, label, name) in rows {
                    let response = ui.selectable_label(self.pod_selected == Some(index), label);
                    #[cfg(test)]
                    self.pod_rows.insert(index, response.rect);
                    if response.clicked() {
                        if self.pod_selected != Some(index) {
                            self.pod_cab_pending = None;
                            self.pod_delay_model_pending = None;
                            self.pod_mod_model_pending = None;
                            self.pod_harmony_pending = None;
                            self.pod_delay_pending = None;
                            self.pod_mod_pending = None;
                        }
                        self.pod_selected = Some(index);
                        self.pod_name = name;
                    }
                }
                let amp_choice_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).and_then(|tone| {
                        if tone.container() != pod_preset::ContainerKind::L6pa {
                            return None;
                        }
                        Some((index, tone.amp_model_id()))
                    })
                });
                if let Some((index, current)) = amp_choice_state {
                    let mut apply_choice = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "AMP model (offline) · current {}",
                            current.map_or_else(
                                || "unavailable".to_owned(),
                                |id| format!("0x{id:08x}")
                            )
                        ));
                        for (id, choice, label) in [
                            ("pod-amp-no-amp", pod_preset::PodAmpModelChoice::NoAmp, "Apply No Amp · 00020000"),
                            ("pod-amp-line6-clean", pod_preset::PodAmpModelChoice::Line6Clean, "Apply Line 6 Clean · 00020001"),
                        ] {
                            if self.pod_button(ui, id, label) {
                                apply_choice = Some(choice);
                            }
                        }
                    });
                    if let Some(choice) = apply_choice {
                        self.pod_selected = Some(index);
                        self.apply_pod_amp_model_choice(choice);
                    }
                }
                let cab_state = self.pod_selected.and_then(|index| self.pod_library.tone(index).and_then(|tone| {
                    (tone.container() == pod_preset::ContainerKind::L6as && tone.cab_model_id().is_some())
                        .then_some((index, tone.cab_model_id(), tone.cab_class(), tone.mic_choice()))
                }));
                if let Some((index, cab_id, class, mic)) = cab_state {
                    let fallback = pod_preset::PodCabModelChoice::from_model_id(
                        pod_preset::PODXT_CAB_MODEL_IDS[0],
                    )
                    .expect("source CAB allowlist is nonempty");
                    let mut selected_cab = self
                        .pod_cab_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                        .map(|(_, choice)| choice)
                        .or_else(|| {
                            cab_id.and_then(pod_preset::PodCabModelChoice::from_model_id)
                        })
                        .unwrap_or(fallback);
                    ui.label(format!("CAB (offline) · current {} · MIC class {}", cab_id.map_or_else(|| "unavailable".into(), |id| format!("{id:08x}")), match class { Some(pod_preset::PodCabClass::Bass) => "bass", Some(pod_preset::PodCabClass::Other) => "standard", None => "unknown" }));
                    ui.horizontal(|ui| {
                        #[cfg(test)]
                        let mut option_rects = Vec::new();
                        let _combo = egui::ComboBox::from_id_salt("pod-cab-choice")
                            .selected_text(selected_cab.numeric_label())
                            .show_ui(ui, |ui| {
                                for id in pod_preset::PODXT_CAB_MODEL_IDS {
                                    if let Some(choice) = pod_preset::PodCabModelChoice::from_model_id(id) {
                                        let _response = ui.selectable_value(&mut selected_cab, choice, format!("CAB {id:08x}"));
                                        #[cfg(test)]
                                        option_rects.push((id, _response.rect));
                                    }
                                }
                            });
                        #[cfg(test)]
                        {
                            self.pod_cab_combo = Some(_combo.response.rect);
                            self.pod_cab_options.extend(option_rects);
                        }
                        self.pod_cab_pending = Some((index, selected_cab));
                        if self.pod_button(ui, "pod-cab-apply", "Apply CAB") {
                            self.pod_selected = Some(index);
                            self.apply_pod_cab_model_choice(selected_cab);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("MIC current {}", mic.map_or_else(|| "unknown".into(), |m| m.value().to_string())));
                        for value in 0..=3 {
                            let id = ["pod-mic-0", "pod-mic-1", "pod-mic-2", "pod-mic-3"][value as usize];
                            if self.pod_button(ui, id, &format!("Apply MIC {value}")) {
                                self.pod_selected = Some(index);
                                if let Some(choice) = pod_preset::PodMicChoice::from_value(value) { self.apply_pod_mic_choice(choice); }
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("PINF name");
                    ui.text_edit_singleline(&mut self.pod_name);
                });
                let controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::AmpControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.amp_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !controls.is_empty() {
                    ui.small(pod_preset::AMP_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in controls {
                    ui.horizontal(|ui| {
                        ui.label(control.label());
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::AMP_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        if control == pod_preset::AmpControl::Drive {
                            self.pod_drive_slider = Some(response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_amp_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Amp {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let stomp_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::StompControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.stomp_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !stomp_controls.is_empty() {
                    ui.small(pod_preset::STOMP_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in stomp_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Stomp {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::STOMP_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_stomp_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_stomp_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Stomp {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let whammy_knob_positions = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index).map(|tone| (index, tone)))
                    .map(|(index, tone)| {
                        pod_preset::StompWhammyKnob::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.stomp_whammy_knob_position(control)
                                    .ok()
                                    .map(|position| (index, control, position))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !whammy_knob_positions.is_empty() {
                    ui.small(
                        "Line6Edit normalized knob position 0–1; source-quantized, not device-validated",
                    );
                }
                for (index, control, mut position) in whammy_knob_positions {
                    ui.horizontal(|ui| {
                        ui.label(format!("Stomp Whammy {} knob position", control.label()));
                        let response = ui
                            .add(
                                egui::Slider::new(&mut position, 0.0..=1.0).show_value(true),
                            )
                            .on_hover_text(
                                "Normalized Line6Edit knob position; source quantization is applied. No physical units or device acceptance are claimed.",
                            );
                        #[cfg(test)]
                        self.pod_whammy_knob_sliders
                            .insert(control, response.rect);
                        if response.changed() {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_stomp_whammy_knob_position(control, position)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Stomp Whammy {} knob position. Source quantization applied. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let eq_bands = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index).map(|tone| (index, tone)))
                    .map(|(index, tone)| {
                        pod_preset::PodxtEqBand::ALL
                            .into_iter()
                            .filter_map(|band| {
                                tone.podxt_eq_band_display_values(band)
                                    .ok()
                                    .map(|(frequency, gain)| (index, band, frequency, gain))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !eq_bands.is_empty() {
                    ui.small("PODxt four-band EQ graph values from Line6Edit 3.06; offline edit only. No device acceptance or audio behavior is claimed.");
                }
                for (index, band, mut frequency, mut gain) in eq_bands {
                    let table = match band {
                        pod_preset::PodxtEqBand::One => (50.0, 690.0),
                        pod_preset::PodxtEqBand::Two => (50.0, 6050.0),
                        pod_preset::PodxtEqBand::Three => (100.0, 11300.0),
                        pod_preset::PodxtEqBand::Four => (500.0, 9300.0),
                    };
                    ui.label(format!("EQ band {}", match band { pod_preset::PodxtEqBand::One => 1, pod_preset::PodxtEqBand::Two => 2, pod_preset::PodxtEqBand::Three => 3, pod_preset::PodxtEqBand::Four => 4 }));
                    ui.horizontal(|ui| {
                        let frequency_response = ui.add(egui::Slider::new(&mut frequency, table.0..=table.1).text("Frequency").suffix(" Hz"));
                        #[cfg(test)]
                        self.pod_eq_frequency_sliders.insert(band, frequency_response.rect);
                        let gain_response = ui.add(egui::Slider::new(&mut gain, -12.8..=12.6).text("Gain").suffix(" dB"));
                        #[cfg(test)]
                        self.pod_eq_gain_sliders.insert(band, gain_response.rect);
                        if frequency_response.changed() || gain_response.changed() {
                            match self.edit_pod_tone(index, |tone| tone.set_podxt_eq_band(band, frequency, gain)) {
                                Ok(()) => self.pod_message = format!("Applied EQ band {} graph edit. Nothing was sent.", match band { pod_preset::PodxtEqBand::One => 1, pod_preset::PodxtEqBand::Two => 2, pod_preset::PodxtEqBand::Three => 3, pod_preset::PodxtEqBand::Four => 4 }),
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let stomp_wave = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_wave_selection().ok());
                if let Some(current) = stomp_wave {
                    let previous = match current {
                        pod_preset::StompWaveValue::Known(choice) => Some(choice),
                        pod_preset::StompWaveValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::StompWaveValue::Known(choice) => choice.label().to_owned(),
                        pod_preset::StompWaveValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Wave");
                        if matches!(current, pod_preset::StompWaveValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::StompWaveSelection::ALL {
                            let _response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_stomp_wave_choices.insert(choice, _response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_stomp_wave_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Stomp Wave {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let harmony1 = self.pod_selected.and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_harmony_1m335_selection().ok());
                if let Some(current) = harmony1 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Harmony (-1/Maj3/5)");
                        match current { pod_preset::StompHarmonyValue::Known(choice) => { ui.small(choice.label()); }, pod_preset::StompHarmonyValue::Unknown { raw_bits } => { ui.small(format!("Unknown float 0x{raw_bits:08x}")); } }
                        for choice in pod_preset::StompHarmony1M335Selection::ALL {
                            let previous = match self.pod_harmony_pending { Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))) if Some(index) == self.pod_selected => Some(choice), _ => match current { pod_preset::StompHarmonyValue::Known(choice) => Some(choice), _ => None } };
                            let mut selected = previous;
                            let _response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)] self.pod_harmony1_choices.insert(choice, _response.rect);
                            if selected != previous && let Some(index) = self.pod_selected { self.pod_harmony_pending = Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))); }
                        }
                    });
                    if let Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))) = self.pod_harmony_pending.filter(|(index, _, _)| Some(*index) == self.pod_selected) {
                        ui.label(format!("Pending {} (not applied)", choice.label()));
                        if self.pod_button(ui, "pod-harmony1-apply", "Apply Harmony 1") {
                            match self.apply_pod_stomp_harmony(index, pod_preset::StompHarmonySelection::Harmony1M335(choice)) {
                                Ok(()) => { self.pod_harmony_pending = None; self.pod_message = format!("Applied Stomp HARMONY_1M335 {}. Nothing was sent.", choice.label()); }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    }
                }
                let harmony2 = self.pod_selected.and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_harmony_1457_selection().ok());
                if let Some(current) = harmony2 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Harmony (-1/2/4/7)");
                        match current { pod_preset::StompHarmonyValue::Known(choice) => { ui.small(choice.label()); }, pod_preset::StompHarmonyValue::Unknown { raw_bits } => { ui.small(format!("Unknown float 0x{raw_bits:08x}")); } }
                        for choice in pod_preset::StompHarmony1457Selection::ALL {
                            let previous = match self.pod_harmony_pending { Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))) if Some(index) == self.pod_selected => Some(choice), _ => match current { pod_preset::StompHarmonyValue::Known(choice) => Some(choice), _ => None } };
                            let mut selected = previous;
                            let _response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)] self.pod_harmony2_choices.insert(choice, _response.rect);
                            if selected != previous && let Some(index) = self.pod_selected { self.pod_harmony_pending = Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))); }
                        }
                    });
                    if let Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))) = self.pod_harmony_pending.filter(|(index, _, _)| Some(*index) == self.pod_selected) {
                        ui.label(format!("Pending {} (not applied)", choice.label()));
                        if self.pod_button(ui, "pod-harmony2-apply", "Apply Harmony 2") {
                            match self.apply_pod_stomp_harmony(index, pod_preset::StompHarmonySelection::Harmony1457(choice)) {
                                Ok(()) => { self.pod_harmony_pending = None; self.pod_message = format!("Applied Stomp HARMONY_1457 {}. Nothing was sent.", choice.label()); }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    }
                }
                let mod_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::ModControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.mod_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !mod_controls.is_empty() {
                    ui.small(pod_preset::MOD_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in mod_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Mod {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::MOD_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_mod_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_mod_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Mod {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let reverb_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::ReverbControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.reverb_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !reverb_controls.is_empty() {
                    ui.small(pod_preset::REVERB_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in reverb_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Reverb {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::REVERB_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_reverb_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_reverb_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Reverb {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let delay_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::DelayControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.delay_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !delay_controls.is_empty() {
                    ui.small(pod_preset::DELAY_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in delay_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Delay {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::DELAY_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_delay_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_delay_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Delay {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let delay_note_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        (index, tone.delay_note_value(), tone.delay_time_ms())
                    })
                });
                let delay_model_state = self.pod_selected.and_then(|index| self.pod_library.tone(index).and_then(|tone| {
                    (tone.container() == pod_preset::ContainerKind::L6pa).then_some((index, tone.models().iter().find(|m| m.slot == pod_preset::DELAY_SLOT || m.slot == 0x0005_0004).map(|m| m.model_id)))
                }));
                if let Some((index, current)) = delay_model_state {
                    let mut selected = self.pod_delay_model_pending
                        .filter(|(i, _)| *i == index)
                        .map(|(_, choice)| Some(choice))
                        .or_else(|| current.and_then(pod_preset::PodDelayModelChoice::from_model_id).map(Some))
                        .unwrap_or(None);
                    let selected_text = selected
                        .map(pod_preset::PodDelayModelChoice::numeric_label)
                        .unwrap_or_else(|| current.map_or_else(|| "Select model".into(), |id| format!("{id:08x} (outside choices)")));
                    ui.separator();
                    ui.label(format!("Delay model ID (offline; stored parameters are retained, no reset) · current {}", current.map_or_else(|| "unavailable".into(), |id| format!("{id:08x}"))));
                    let mut changed = None;
                    let _combo = egui::ComboBox::from_id_salt("pod-delay-model-choice")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for id in pod_preset::PodDelayModelChoice::MODEL_IDS {
                                let choice = pod_preset::PodDelayModelChoice::from_model_id(id)
                                    .expect("listed Delay model id is admitted");
                                let response = ui.selectable_value(&mut selected, Some(choice), format!("{id:08x}"));
                                #[cfg(test)] { self.pod_delay_model_options.insert(id, response.rect); }
                                if response.changed() { changed = Some(choice); }
                            }
                        });
                    #[cfg(test)] { self.pod_delay_model_combo = Some(_combo.response.rect); }
                    if let Some(choice) = changed { self.pod_delay_model_pending = Some((index, choice)); }
                    if let Some((pending_index, choice)) = self.pod_delay_model_pending.filter(|(i, _)| *i == index) {
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {} (parameters retained)", choice.numeric_label()));
                            let response = ui.button("Apply Delay model");
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-model-apply", response.rect); }
                            if response.clicked() {
                                match self.apply_pod_delay_model_choice(pending_index, choice) {
                                    Ok(()) => { self.pod_delay_model_pending = None; self.pod_message = "Applied the Delay model ID offline; stored parameters were retained. Nothing was sent.".into(); }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-model-discard", discard.rect); }
                            if discard.clicked() { self.pod_delay_model_pending = None; }
                        });
                    }
                }
                if let Some((index, note_result, time_result)) = delay_note_state {
                    ui.separator();
                    ui.label("Delay NOTE / TIME");
                    match note_result {
                        Ok(value) => {
                            let selected_current = match value {
                                pod_preset::DelayNoteValue::Known(choice) => Some(choice),
                                pod_preset::DelayNoteValue::Unknown { .. } => None,
                            };
                            let selected_pending = match self.pod_delay_pending {
                                Some((pending_index, PendingDelayEdit::Note(choice))) if pending_index == index => Some(choice),
                                _ => selected_current,
                            };
                            let selected_label = match value {
                                pod_preset::DelayNoteValue::Known(choice) => choice.label().to_owned(),
                                pod_preset::DelayNoteValue::Unknown { raw_bits } => format!("Unknown NOTE {raw_bits}"),
                            };
                            ui.label(format!("Stored: {selected_label}"));
                            let mut selected = selected_pending;
                            let mut changed = None;
                            ui.horizontal_wrapped(|ui| {
                                for choice in pod_preset::DelayNoteChoice::ALL {
                                    let response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                                    #[cfg(test)]
                                    self.pod_delay_note_options.insert(choice, response.rect);
                                    if response.changed() { changed = selected; }
                                }
                            });
                            if let Some(choice) = changed {
                                self.pod_delay_pending = Some((index, PendingDelayEdit::Note(choice)));
                            }
                        }
                        Err(error) => { ui.small(format!("Delay NOTE unavailable: {}", error.detail())); }
                    }
                    match time_result {
                        Ok(current_ms) => {
                            let pending_ms = match self.pod_delay_pending {
                                Some((pending_index, PendingDelayEdit::TimeMs(value))) if pending_index == index => value,
                                _ => current_ms,
                            };
                            let mut value = pending_ms;
                            ui.horizontal(|ui| {
                                ui.label("TIME (ms)");
                                let response = ui.add(egui::Slider::new(&mut value, 20.0..=2000.0).show_value(true));
                                #[cfg(test)] { self.pod_delay_time_slider = Some(response.rect); }
                                if response.changed() {
                                    self.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(value)));
                                }
                            });
                        }
                        Err(error) => { ui.small(format!("Delay TIME unavailable: {}", error.detail())); }
                    }
                    if let Some((pending_index, edit)) = self.pod_delay_pending.filter(|(pending_index, _)| *pending_index == index) {
                        let (label, button) = match edit {
                            PendingDelayEdit::Note(choice) => (format!("{} (not applied)", choice.label()), "Apply Delay NOTE"),
                            PendingDelayEdit::TimeMs(value) => (format!("{value:.3} ms (not applied)"), "Apply Delay TIME"),
                        };
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {label}"));
                            let response = ui.button(button);
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-apply", response.rect); }
                            if response.clicked() {
                                match self.apply_pod_delay_edit(pending_index, edit) {
                                    Ok(()) => {
                                        self.pod_delay_pending = None;
                                        self.pod_message = "Applied the staged Delay NOTE/TIME edit offline. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons
                                .insert("pod-delay-discard", discard.rect);
                            if discard.clicked() { self.pod_delay_pending = None; }
                        });
                    }
                }
                let mod_model_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).and_then(|tone| {
                        (tone.container() == pod_preset::ContainerKind::L6pa).then_some((
                            index,
                            tone.models()
                                .iter()
                                .find(|model| {
                                    model.slot == pod_preset::MOD_SLOT || model.slot == 0x0005_0003
                                })
                                .map(|model| model.model_id),
                        ))
                    })
                });
                if let Some((index, current)) = mod_model_state {
                    let mut selected = self
                        .pod_mod_model_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                        .map(|(_, choice)| Some(choice))
                        .or_else(|| {
                            current
                                .and_then(pod_preset::PodModModelChoice::from_model_id)
                                .map(Some)
                        })
                        .unwrap_or(None);
                    let selected_text = selected
                        .map(pod_preset::PodModModelChoice::numeric_label)
                        .unwrap_or_else(|| {
                            current.map_or_else(
                                || "Select model".into(),
                                |id| format!("{id:08x} (outside choices)"),
                            )
                        });
                    ui.separator();
                    ui.label(format!(
                        "Mod model ID (offline; parameters retained, no reset) · current {}",
                        current.map_or_else(|| "unavailable".into(), |id| format!("{id:08x}"))
                    ));
                    let mut changed = None;
                    let combo = egui::ComboBox::from_id_salt("pod-mod-model-choice")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for id in pod_preset::PodModModelChoice::MODEL_IDS {
                                let choice = pod_preset::PodModModelChoice::from_model_id(id)
                                    .expect("listed Mod model id is admitted");
                                let response = ui.selectable_value(
                                    &mut selected,
                                    Some(choice),
                                    format!("{id:08x}"),
                                );
                                #[cfg(test)]
                                self.pod_mod_model_options.insert(id, response.rect);
                                if response.changed() {
                                    changed = Some(choice);
                                }
                            }
                        });
                    #[cfg(test)]
                    {
                        self.pod_mod_model_combo = Some(combo.response.rect);
                    }
                    #[cfg(not(test))]
                    let _ = combo;
                    if let Some(choice) = changed {
                        self.pod_mod_model_pending = Some((index, choice));
                    }
                    if let Some((pending_index, choice)) = self
                        .pod_mod_model_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                    {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Pending {} (parameters retained)",
                                choice.numeric_label()
                            ));
                            let apply = ui.button("Apply Mod model");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-model-apply", apply.rect);
                            if apply.clicked() {
                                match self.apply_pod_mod_model_choice(pending_index, choice) {
                                    Ok(()) => {
                                        self.pod_mod_model_pending = None;
                                        self.pod_message = "Applied the Mod model ID offline; stored parameters were retained. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-model-discard", discard.rect);
                            if discard.clicked() {
                                self.pod_mod_model_pending = None;
                            }
                        });
                    }
                }
                let mod_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        (index, tone.mod_note_value(), tone.mod_speed_hz())
                    })
                });
                if let Some((index, note_result, speed_result)) = mod_state {
                    ui.separator();
                    ui.label("Mod NOTE / SPEED");
                    match note_result {
                        Ok(value) => {
                            let current = match value {
                                pod_preset::ModNoteValue::Known(choice) => Some(choice),
                                pod_preset::ModNoteValue::Unknown { .. } => None,
                            };
                            let pending = match self.pod_mod_pending {
                                Some((pending_index, PendingModEdit::Note(choice))) if pending_index == index => Some(choice),
                                _ => current,
                            };
                            let label = match value {
                                pod_preset::ModNoteValue::Known(choice) => choice.label().to_owned(),
                                pod_preset::ModNoteValue::Unknown { raw_bits } => format!("Unknown NOTE {raw_bits}"),
                            };
                            ui.label(format!("Stored: {label}"));
                            let mut selected = pending;
                            let mut changed = None;
                            ui.horizontal_wrapped(|ui| {
                                for choice in pod_preset::DelayNoteChoice::ALL {
                                    let response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                                    #[cfg(test)]
                                    self.pod_mod_note_options.insert(choice, response.rect);
                                    if response.changed() { changed = selected; }
                                }
                            });
                            if let Some(choice) = changed {
                                self.pod_mod_pending = Some((index, PendingModEdit::Note(choice)));
                            }
                        }
                        Err(error) => { ui.small(format!("Mod NOTE unavailable: {}", error.detail())); }
                    }
                    match speed_result {
                        Ok(current_hz) => {
                            let pending_hz = match self.pod_mod_pending {
                                Some((pending_index, PendingModEdit::SpeedHz(value))) if pending_index == index => value,
                                _ => current_hz,
                            };
                            let mut value = pending_hz;
                            ui.horizontal(|ui| {
                                ui.label("SPEED (Hz)");
                                let response = ui.add(egui::Slider::new(&mut value, 0.1..=15.0).show_value(true));
                                #[cfg(test)] { self.pod_mod_speed_slider = Some(response.rect); }
                                if response.changed() {
                                    self.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(value)));
                                }
                            });
                        }
                        Err(error) => { ui.small(format!("Mod SPEED unavailable: {}", error.detail())); }
                    }
                    if let Some((pending_index, edit)) = self.pod_mod_pending.filter(|(pending_index, _)| *pending_index == index) {
                        let (label, button) = match edit {
                            PendingModEdit::Note(choice) => (format!("{} (not applied)", choice.label()), "Apply Mod NOTE"),
                            PendingModEdit::SpeedHz(value) => (format!("{value:.3} Hz (not applied)"), "Apply Mod SPEED"),
                        };
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {label}"));
                            let apply = ui.button(button);
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-apply", apply.rect);
                            if apply.clicked() {
                                match self.apply_pod_mod_edit(pending_index, edit) {
                                    Ok(()) => {
                                        self.pod_mod_pending = None;
                                        self.pod_message = "Applied the staged Mod NOTE/SPEED edit offline. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-discard", discard.rect);
                            if discard.clicked() { self.pod_mod_pending = None; }
                        });
                    }
                }
                let delay_heads = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.delay_heads_selection().ok());
                if let Some(current) = delay_heads {
                    let previous = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => Some(choice),
                        pod_preset::DelayAssociationValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => {
                            choice.label().to_owned()
                        }
                        pod_preset::DelayAssociationValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Delay Heads");
                        if matches!(current, pod_preset::DelayAssociationValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::DelayHeadsSelection::ALL {
                            let _choice_response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_delay_heads_choices
                                .insert(choice, _choice_response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_delay_heads_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Delay Heads {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let delay_bits = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.delay_bits_selection().ok());
                if let Some(current) = delay_bits {
                    let previous = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => Some(choice),
                        pod_preset::DelayAssociationValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => {
                            choice.label().to_owned()
                        }
                        pod_preset::DelayAssociationValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Delay Bits");
                        if matches!(current, pod_preset::DelayAssociationValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::DelayBitsSelection::ALL {
                            let _choice_response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_delay_bits_choices.insert(choice, _choice_response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_delay_bits_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Delay Bits {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let detail = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        format!(
                            "Selected {} version {:08x}. {} slots have no GPXT role. {} parameter records have no known range. {} parameter chunks stay opaque.\n{}",
                            tone.container().tag(),
                            tone.device_version(),
                            tone.undecoded_slots(),
                            tone.recorded_parameter_chunks(),
                            tone.undecoded_parameter_chunks(),
                            tone.control_lines().join("\n"),
                        )
                    })
                });
                if let Some(detail) = detail {
                    ui.label(detail);
                }
                #[cfg(test)]
                {
                    let selected = self.pod_selected.and_then(|index| {
                        self.pod_library.tone(index).map(|tone| {
                            format!(
                                "\n{} {} amp {} cab {} undecoded {}\n{}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                                tone.undecoded_slots(),
                                tone.control_lines().join("\n"),
                            )
                        })
                    });
                    self.pod_visible = format!(
                        "{}\n{}\n{}\n{}\n{}",
                        pod_preset::OFFLINE_BANNER,
                        pod_preset::NOT_GUITAR,
                        pod_preset::NOT_SENT,
                        pod_preset::PAYLOADS_NOT_DECODED,
                        self.pod_message,
                    );
                    if let Some(selected) = selected {
                        self.pod_visible.push_str(&selected);
                    }
                }
            });
        // Closing this window keeps the in-memory library, including unsaved names.
        self.pod_open = open;
    }

    fn pod_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.pod_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn pod_button_enabled(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: &str,
        enabled: bool,
    ) -> bool {
        let response = ui.add_enabled(enabled, egui::Button::new(label));
        #[cfg(test)]
        self.pod_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn model_enable_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        ui.label("Stored model flags only; no bypass, audio, or device effect is claimed.");
        for control in [
            pod_preset::PodModelEnable::Amp,
            pod_preset::PodModelEnable::Eq,
        ] {
            let state = self
                .pod_library
                .tone(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))
                .and_then(|tone| tone.model_enable_state(control));
            match state {
                Ok(pod_preset::ModelEnableState::Canonical(mut enabled)) => {
                    let label = match control {
                        pod_preset::PodModelEnable::Amp => "Amp enabled (stored flag)",
                        pod_preset::PodModelEnable::Eq => "EQ enabled (stored flag)",
                    };
                    let response = ui.checkbox(&mut enabled, label);
                    #[cfg(test)]
                    self.pod_model_enable_checks.insert(control, response.rect);
                    if response.changed() {
                        match self
                            .edit_pod_tone(index, |tone| tone.set_model_enabled(control, enabled))
                        {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied {} stored enable flag. Nothing was sent.",
                                    match control {
                                        pod_preset::PodModelEnable::Amp => "Amp",
                                        pod_preset::PodModelEnable::Eq => "EQ",
                                    }
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                Ok(pod_preset::ModelEnableState::NonCanonical {
                    raw,
                    source_interpretation,
                }) => {
                    let label = match control {
                        pod_preset::PodModelEnable::Amp => "Amp",
                        pod_preset::PodModelEnable::Eq => "EQ",
                    };
                    ui.label(format!(
                        "{label} edit unavailable: MINF byte 0x{raw:02x} is noncanonical (Line6Edit reads it as {}). Preserved unchanged.",
                        if source_interpretation { "enabled" } else { "disabled" }
                    ));
                }
                Err(error) => {
                    let label = match control {
                        pod_preset::PodModelEnable::Amp => "Amp",
                        pod_preset::PodModelEnable::Eq => "EQ",
                    };
                    ui.label(format!("{label} edit unavailable: {}", error.detail()));
                }
            }
        }
    }

    fn edit_pod_tone(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut pod_preset::PodTone) -> Result<(), pod_preset::ToneError>,
    ) -> Result<(), pod_preset::ToneError> {
        if let Some(bundle) = &mut self.pod_bundle {
            bundle.edit_entry(index, edit)?;
            let (bank, preset) = bundle
                .entry_location(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?;
            let bytes = bundle.tone(bank, preset)?.as_bytes().to_vec();
            self.pod_library.replace(index, bytes)
        } else {
            let mut tone =
                self.pod_library.tone(index).cloned().ok_or_else(|| {
                    pod_preset::ToneError::format("POD selection is out of range")
                })?;
            edit(&mut tone)?;
            self.pod_library.replace(index, tone.into_bytes())
        }
    }

    fn replace_pod_tone(
        &mut self,
        index: usize,
        bytes: Vec<u8>,
    ) -> Result<(), pod_preset::ToneError> {
        let mut library = self.pod_library.clone();
        library.replace(index, bytes.clone())?;
        let mut bundle = self.pod_bundle.clone();
        if let Some(bundle) = &mut bundle {
            bundle.replace_entry(index, bytes)?;
        }
        self.pod_library = library;
        self.pod_bundle = bundle;
        Ok(())
    }

    fn apply_pod_stomp_harmony(
        &mut self,
        index: usize,
        selection: pod_preset::StompHarmonySelection,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_stomp_harmony_selection(RoleKind::Pod, &current, selection)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn apply_pod_delay_edit(
        &mut self,
        index: usize,
        edit: PendingDelayEdit,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let session = Session::initial_rig();
        let replacement = match edit {
            PendingDelayEdit::Note(choice) => {
                session.set_pod_delay_note_choice(RoleKind::Pod, &current, choice)
            }
            PendingDelayEdit::TimeMs(milliseconds) => {
                session.set_pod_delay_time_ms(RoleKind::Pod, &current, milliseconds)
            }
        }
        .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn apply_pod_mod_edit(
        &mut self,
        index: usize,
        edit: PendingModEdit,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let session = Session::initial_rig();
        let replacement = match edit {
            PendingModEdit::Note(choice) => {
                session.set_pod_mod_note_choice(RoleKind::Pod, &current, choice)
            }
            PendingModEdit::SpeedHz(hz) => {
                session.set_pod_mod_speed_hz(RoleKind::Pod, &current, hz)
            }
        }
        .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn load_pod_bundle_bytes(&mut self, bytes: Vec<u8>) -> Result<(), pod_preset::ToneError> {
        if self.pod_dirty() {
            return Err(pod_preset::ToneError::format(
                "save or discard POD edits before loading a bank",
            ));
        }
        let bundle = pod_preset::PodBundleLibrary::parse(bytes)?;
        let mut library = Library::default();
        for index in 0..bundle.entry_count() {
            let (bank_index, preset_index) = bundle
                .entry_location(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD bundle index became invalid"))?;
            library.import_bytes(bundle.tone(bank_index, preset_index)?.into_bytes())?;
        }
        self.pod_library = library;
        self.pod_bundle = Some(bundle);
        self.pod_selected = Some(0);
        self.pod_cab_pending = None;
        self.pod_harmony_pending = None;
        self.refresh_selected_pod_name();
        Ok(())
    }

    fn refresh_pod_library_from_bundle(&mut self) -> Result<(), pod_preset::ToneError> {
        let mut library = Library::default();
        if let Some(bundle) = &self.pod_bundle {
            for index in 0..bundle.entry_count() {
                let (bank_index, preset_index) = bundle.entry_location(index).ok_or_else(|| {
                    pod_preset::ToneError::format("POD bundle index became invalid")
                })?;
                library.import_bytes(bundle.tone(bank_index, preset_index)?.into_bytes())?;
            }
        }
        self.pod_library = library;
        self.pod_cab_pending = None;
        self.pod_harmony_pending = None;
        Ok(())
    }

    fn apply_pod_membership_action(
        &mut self,
        action: PodMembershipAction,
        bank: usize,
        preset: usize,
    ) {
        let Some(mut candidate) = self.pod_bundle.clone() else {
            return;
        };
        let folder_editable = candidate.membership_editable(bank);
        let result = match action {
            PodMembershipAction::Duplicate => candidate.duplicate_entry(bank, preset),
            PodMembershipAction::Remove => candidate.remove_entry(bank, preset),
            PodMembershipAction::MoveUp if preset > 0 => {
                if folder_editable {
                    candidate.reorder_entry(bank, preset, preset - 1)
                } else {
                    candidate.reorder_fixed_bank_entry(bank, preset, preset - 1)
                }
            }
            PodMembershipAction::MoveDown => {
                if folder_editable {
                    candidate.reorder_entry(bank, preset, preset + 1)
                } else {
                    candidate.reorder_fixed_bank_entry(bank, preset, preset + 1)
                }
            }
            PodMembershipAction::MoveUp => return,
        }
        .and_then(|selected| {
            let mut library = Library::default();
            for index in 0..candidate.entry_count() {
                let (bank_index, preset_index) =
                    candidate.entry_location(index).ok_or_else(|| {
                        pod_preset::ToneError::format("POD bundle index became invalid")
                    })?;
                library.import_bytes(candidate.tone(bank_index, preset_index)?.into_bytes())?;
            }
            Ok((selected, library))
        });
        match result {
            Ok((selected, library)) => {
                self.pod_bundle = Some(candidate);
                self.pod_library = library;
                self.pod_selected = Some(selected);
                self.refresh_selected_pod_name();
                self.pod_message = match action {
                    PodMembershipAction::Duplicate => {
                        "Duplicated the existing tone bytes in this folder. Nothing was sent."
                            .into()
                    }
                    PodMembershipAction::Remove => {
                        "Removed the selected tone from this folder. Nothing was sent.".into()
                    }
                    PodMembershipAction::MoveUp | PodMembershipAction::MoveDown => {
                        if folder_editable {
                            "Reordered tones in this folder. Nothing was sent.".into()
                        } else {
                            "Reordered tones in this fixed bank. Nothing was sent.".into()
                        }
                    }
                };
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn load_pod_tone(&mut self) {
        let Some(path) = self.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        match pod_preset::read_regular_bundle(pod_preset::path_facts(&path), || {
            let file = fs::File::open(&path)?;
            let metadata = file.metadata()?;
            Ok((
                pod_preset::PathFacts {
                    regular_file: metadata.is_file(),
                    len: metadata.len(),
                },
                file,
            ))
        }) {
            Ok(bytes) if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) => {
                match self.load_pod_bundle_bytes(bytes) {
                    Ok(()) => {
                        let (banks, tones) = self
                            .pod_bundle
                            .as_ref()
                            .map_or((0, 0), |bundle| (bundle.bank_count(), bundle.entry_count()));
                        self.pod_message = format!(
                            "Loaded PODxt Live collection with {banks} banks and {tones} tones. Offline edits only; nothing was sent."
                        );
                    }
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            Ok(bytes) => {
                if self.pod_bundle.is_some() && self.pod_dirty() {
                    self.pod_message =
                        "Save or discard bank edits before loading an individual tone.".into();
                    return;
                }
                match Session::initial_rig().open_pod_preset(RoleKind::Pod, &bytes) {
                    Ok(summary) => {
                        if self.pod_bundle.is_some() {
                            self.pod_bundle = None;
                            self.pod_library.clear();
                            self.pod_selected = None;
                            self.pod_cab_pending = None;
                            self.pod_harmony_pending = None;
                        }
                        match self.pod_library.import_bytes(bytes) {
                            Ok(index) => {
                                self.pod_selected = Some(index);
                                self.pod_cab_pending = None;
                                self.pod_harmony_pending = None;
                                self.pod_name = summary.name;
                                self.pod_message = format!(
                                    "Loaded {} for PODxt Live {:08x}. {}",
                                    summary.container,
                                    summary.device_id,
                                    pod_preset::PAYLOADS_NOT_DECODED
                                );
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                    Err(error) => self.pod_message = rejection_text(&error),
                }
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn apply_pod_name(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before renaming it.".into();
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            self.pod_message = "Select a tone before renaming it.".into();
            return;
        };
        match Session::initial_rig().rename_pod_preset(RoleKind::Pod, &current, &self.pod_name) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Renamed the in-memory tone to {}. Nothing was sent.",
                        self.pod_name
                    );
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_amp_model_choice(&mut self, choice: pod_preset::PodAmpModelChoice) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before applying an AMP model.".into();
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            self.pod_message = "Select a tone before applying an AMP model.".into();
            return;
        };
        match Session::initial_rig().set_pod_amp_model_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Applied AMP model {} (0x{:08x}) to the offline tone. Nothing was sent.",
                        choice.label(),
                        choice.model_id()
                    );
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_delay_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodDelayModelChoice,
    ) -> Result<(), pod_preset::ToneError> {
        let slot_ids = [pod_preset::DELAY_SLOT, 0x0005_0004];
        let before = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slot_ids.contains(&model.slot))
            })
            .map(|model| model.model_id);
        self.edit_pod_tone(index, |tone| tone.set_delay_model_choice(choice))?;
        let after = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slot_ids.contains(&model.slot))
            })
            .map(|model| model.model_id);
        if before != after {
            self.pod_delay_pending = None;
        }
        Ok(())
    }

    fn apply_pod_mod_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodModModelChoice,
    ) -> Result<(), pod_preset::ToneError> {
        let slots = [pod_preset::MOD_SLOT, 0x0005_0003];
        let before = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slots.contains(&model.slot))
            })
            .map(|model| model.model_id);
        let bytes = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_mod_model_choice(RoleKind::Pod, &bytes, choice)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)?;
        let after = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slots.contains(&model.slot))
            })
            .map(|model| model.model_id);
        if before != after {
            self.pod_mod_pending = None;
        }
        Ok(())
    }

    fn apply_pod_cab_model_choice(&mut self, choice: pod_preset::PodCabModelChoice) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            return;
        };
        match Session::initial_rig().set_pod_cab_model_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Applied CAB {}. MIC value was preserved; nothing was sent.",
                        choice.numeric_label()
                    )
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_mic_choice(&mut self, choice: pod_preset::PodMicChoice) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            return;
        };
        match Session::initial_rig().set_pod_mic_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!("Applied MIC {}. Nothing was sent.", choice.value())
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn save_pod_tone(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return;
        };
        self.write_pod_copy(index);
    }

    fn write_pod_copy(&mut self, index: usize) -> bool {
        if self.pod_bundle.is_some() {
            let Some(path) = self.dialogs.save_pod_tone("podxt-live-bank.vwb") else {
                self.pod_message = "Save cancelled. No file was written.".into();
                return false;
            };
            let Some(bundle) = self.pod_bundle.as_ref() else {
                return false;
            };
            match bundle.save_copy(&path) {
                Ok(()) => {
                    if let Some(bundle) = &mut self.pod_bundle {
                        bundle.note_saved();
                    }
                    for tone_index in 0..self.pod_library.len() {
                        if self.pod_library.is_dirty(tone_index) {
                            let _ = self.pod_library.note_saved(tone_index);
                        }
                    }
                    self.pod_message = format!(
                        "Saved a new bank/collection copy at {}. Existing files were not replaced. Nothing was sent.",
                        path.display()
                    );
                    return true;
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    return false;
                }
            }
        }
        let Some((bytes, suggested)) = self
            .pod_library
            .tone(index)
            .map(|tone| (tone.as_bytes().to_vec(), suggested_tone_name(tone.name())))
        else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return false;
        };
        let Some(path) = self.dialogs.save_pod_tone(&suggested) else {
            self.pod_message = "Save cancelled. No file was written.".into();
            return false;
        };
        match Session::initial_rig().save_pod_copy(RoleKind::Pod, &path, &bytes) {
            Ok(()) => match self.pod_library.note_saved(index) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Saved a new copy at {}. Existing files were not replaced. Nothing was sent.",
                        path.display()
                    );
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            },
            Err(error) => {
                self.pod_message = rejection_text(&error);
                false
            }
        }
    }

    fn record_choice(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        selected: bool,
        label: &str,
    ) -> bool {
        let response = ui.selectable_label(selected, label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn record_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn arm_recording(&mut self) {
        let Some(route) = self.recorder.route() else {
            self.recorder
                .note("Select a route, an input, and a clock before starting.");
            return;
        };
        let mut attempts = 0;
        let gate = variax_workbench::rig::Session::initial_rig().gate_offline_recording(
            route,
            recording::OFFLINE_ROUTE_PERIOD_BYTES,
            recording::OFFLINE_ROUTE_PERIODS,
            || {
                attempts += 1;
                Ok(())
            },
        );
        #[cfg(test)]
        {
            self.recording_capture_attempts = attempts;
        }
        if attempts != 0 {
            self.recorder
                .note("Capture attempt ran. The take was not started.");
            return;
        }
        match gate {
            Err(error) => {
                self.recorder
                    .note(format!("Route gate refused the take: {error:?}"));
            }
            Ok(gate)
                if gate.stream_opened || gate.outbound_len != 0 || !gate.capture_stays_locked =>
            {
                self.recorder.note("Route plan opened a stream or produced outbound bytes. The take was not started.");
            }
            Ok(_) => {
                if let Err(error) = self.recorder.start() {
                    self.recorder.note(error.to_string());
                }
            }
        }
    }

    fn load_recording_wav(&mut self) {
        if self.recorder.phase() != recording::Phase::Idle {
            self.recorder
                .note("Cancel the current take before loading a file.");
            return;
        }
        let Some(path) = self.dialogs.open_wav() else {
            self.recorder.note("Load cancelled. No file was read.");
            return;
        };
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio.wav".into());
        match recording::read_regular_bytes(
            recording::path_facts(&path),
            recording::TAKE_BYTES_CEILING,
            || fs::read(&path),
        ) {
            Ok(bytes) => {
                if let Err(error) = self.recorder.load_wav(&label, &bytes) {
                    self.recorder.note(error.to_string());
                }
            }
            Err(recording::TakeError::CapExceeded) => {
                self.recorder
                    .note("That file is larger than the offline take ceiling. It was not loaded.");
            }
            Err(error) => self.recorder.note(error.to_string()),
        }
    }

    fn save_recording(&mut self) -> bool {
        if self.recorder.phase() != recording::Phase::Stopped {
            self.recorder.note("Stop the take before saving.");
            return false;
        }
        let native = self.recorder.take_can_save_aiff();
        let destination = if native {
            self.dialogs.save_aiff("exact-clock-test-take.aiff")
        } else {
            self.dialogs.save_wav("offline-test-take.wav")
        };
        let Some(path) = destination else {
            self.recorder.note("Save cancelled. No file was written.");
            return false;
        };
        let result = if native {
            self.recorder.save_aiff(&path).map(|_| ())
        } else {
            self.recorder.save(&path).map(|_| ())
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                self.recorder.note(error.to_string());
                false
            }
        }
    }
}

impl Drop for Workbench {
    fn drop(&mut self) {
        if let Some((cancel, worker)) = self.read_worker.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = worker.join();
        }
    }
}

const USAGE: &str = "Usage: variax-workbench-gui [FILE | --preset FILE] [--data-dir DIRECTORY] [--artwork EDITOR_IMAGE_FOLDER] [--tab body|pickups|tuning|controls] [--display auto|x11|wayland (Linux)] [--screenshot OUTPUT.png]";

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
    screenshot: Option<PathBuf>,
    tab: Tab,
    display: String,
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    data_root: &Path,
) -> Result<GuiOptions, String> {
    let mut options = GuiOptions {
        preset: None,
        artwork: data_root.join("artwork"),
        screenshot: None,
        tab: Tab::Body,
        display: "auto".into(),
    };
    let mut data_root = data_root.to_owned();
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
            "--data-dir" => data_root = PathBuf::from(value),
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
                options.tab = match value.as_str() {
                    "body" => Tab::Body,
                    "pickups" => Tab::Pickups,
                    "tuning" => Tab::Tuning,
                    "controls" => Tab::Controls,
                    _ => return Err(format!("Unknown editor tab: {value}")),
                }
            }
            _ => unreachable!(),
        }
    }
    options.artwork = artwork.unwrap_or_else(|| data_root.join("artwork"));
    Ok(options)
}

fn default_data_root() -> PathBuf {
    let absolute_env = |key| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    if let Some(path) = absolute_env("VARIAX_WORKBENCH_DATA_DIR") {
        return path;
    }
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

fn main() -> eframe::Result {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let arguments = match parse_options(arguments, &default_data_root()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("variax-workbench")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([960.0, 680.0]),
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
    eframe::run_native(
        "Variax Workbench",
        options,
        Box::new(move |cc| {
            let mut app = Workbench::new(cc, arguments.preset, arguments.artwork);
            app.screenshot = arguments.screenshot;
            app.tab = arguments.tab;
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (egui::Context, Workbench) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let context = egui::Context::default();
        let mut app = Workbench::with_context(
            &context,
            Some(root.join("tests/fixtures/editor-preset.vwm")),
            root.join("assets/original-ui/modules/variax/editor/images"),
        );
        // These tests exercise egui directly, without a display or theme polling.
        app.theme_polled = Instant::now();
        (context, app)
    }

    fn prompt_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| app.pending_dialog(ctx),
        );
    }

    fn click_prompt(context: &egui::Context, app: &mut Workbench, action: &'static str) {
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
    }

    #[test]
    fn dirty_open_cancel_failed_save_retry_and_discard_follow_actual_prompt_buttons() {
        let dir = tempfile::tempdir().unwrap();
        let next = dir.path().join("next.vwm");
        let saved = dir.path().join("saved.vwm");
        let mut preset =
            PresetFile::parse(include_bytes!("../tests/fixtures/editor-preset.vwm").to_vec())
                .unwrap();
        preset.rename("Next document").unwrap();
        std::fs::write(&next, preset.as_bytes()).unwrap();
        let (context, mut app) = app();
        context.style_mut(|s| s.animation_time = 0.);
        app.dialogs = Box::new(FakeDialogs {
            open: [None, Some(next.clone()), Some(next.clone())].into(),
            save: [None, Some(next.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.change(|p| p.rename("Keep me"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        app.open_dialog();
        assert!(app.pending.is_none());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.open_dialog();
        assert!(matches!(app.pending, Some(Pending::Open(_))));
        app.change(|p| p.rename("Behind prompt"));
        app.undo();
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        click_prompt(&context, &mut app, "save"); // Existing destination is refused.
        assert!(app.pending.is_some());
        assert!(app.error.is_some());
        assert_eq!(std::fs::read(&next).unwrap(), preset.as_bytes());
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert_eq!(std::fs::read(saved).unwrap(), before);
        assert_eq!(
            app.document.as_ref().unwrap().preset().name(),
            "Next document"
        );
        assert!(app.error.is_none());
        app.change(|p| p.rename("Discard me"));
        app.open_dialog();
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert_eq!(app.document.as_ref().unwrap().preset().name(), "Discard me");
        app.pending = Some(Pending::Open(next));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "discard");
        assert_eq!(
            app.document.as_ref().unwrap().preset().name(),
            "Next document"
        );
    }

    #[test]
    fn clean_document_save_cancellation_does_not_continue_an_existing_prompt() {
        let (context, mut app) = app();
        context.style_mut(|s| s.animation_time = 0.);
        app.dialogs = Box::new(FakeDialogs {
            save: [None].into(),
            ..Default::default()
        });
        app.pending = Some(Pending::Quit);
        assert!(!app.dirty());
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
    }

    #[test]
    fn failed_open_and_missing_save_directory_preserve_dirty_document_and_history() {
        let (context, mut app) = app();
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.vwb");
        std::fs::write(&bad, b"invalid container").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(bad)].into(),
            save: [Some(dir.path().join("missing/copy.vwm"))].into(),
            ..Default::default()
        });
        app.change(|p| p.rename("Retained"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        assert!(!app.save_dialog());
        assert!(app.error.is_some());
        app.open_dialog();
        app.continue_pending(&context);
        assert!(app.error.is_some());
        assert!(app.dirty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.undo();
        assert!(!app.dirty());
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn close_during_fake_operation_cancels_it_and_retains_document_after_disconnect() {
        let (context, mut app) = app();
        app.change(|p| p.rename("Retained"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (sender, receiver) = std::sync::mpsc::channel();
        app.read_receiver = Some(receiver);
        app.read_worker = Some((cancel.clone(), std::thread::spawn(|| {})));
        app.request_close(&context);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!app.permit_close);
        assert!(app.pending.is_none());
        app.open_dialog();
        assert!(!app.save_dialog()); // Fake adapter panics if invoked.
        drop(sender);
        app.poll_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
        assert!(app.error.as_deref().unwrap().contains("unexpectedly"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        app.continue_pending(&context);
        assert!(app.permit_close);
    }

    #[test]
    fn actual_pickup_slider_drag_is_one_undo_group() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.float_control(ui, PICKUP1, 0x200000, "Position", " in")
                    });
                },
            );
        }
        let (context, mut app) = app();
        context.style_mut(|style| style.spacing.slider_width = 200.);
        app.load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
        );
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        frame(&context, &mut app, vec![]);
        let rect = app.pickup_slider.unwrap();
        let start = Pos2::new(rect.left() + 30., rect.center().y);
        let end = start + Vec2::new(90., 0.);
        frame(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        for pos in [start + Vec2::new(40., 0.), end] {
            frame(&context, &mut app, vec![egui::Event::PointerMoved(pos)]);
        }
        frame(
            &context,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        let changed = app.document.as_ref().unwrap().as_bytes().to_vec();
        assert_ne!(changed, before);
        app.undo();
        let doc = app.document.as_ref().unwrap();
        assert_eq!(doc.as_bytes(), before);
        assert!(!doc.can_undo());
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), changed);
    }

    #[test]
    fn artwork_dialog_cancel_and_folder_recovery_preserve_document() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let old = app.artwork.clone();
        app.dialogs = Box::new(FakeDialogs {
            artwork: [None, Some(directory.path().to_owned())].into(),
            ..Default::default()
        });
        assert!(app.texture(&context, "missing-test.png").is_none());
        app.artwork_dialog();
        assert_eq!(app.artwork, old);
        assert!(app.error.is_none());
        assert_eq!(app.missing_textures.len(), 1);
        app.artwork_dialog();
        assert_eq!(app.artwork, directory.path());
        assert!(app.error.is_none());
        assert!(app.missing_textures.is_empty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.dirty());
    }

    #[test]
    fn active_read_blocks_document_mutations_and_shortcuts() {
        let (context, mut app) = app();
        app.change(|preset| preset.rename("Before read"));
        let before = app.document.as_ref().unwrap().preset().as_bytes().to_vec();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.read_receiver = Some(receiver);
        app.change(|preset| preset.rename("Must not replace it"));
        app.undo();
        app.redo();
        app.load(Path::new("this-file-must-not-be-opened.vwm"));
        app.open_dialog();
        app.save_dialog();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120.0, 760.0))),
            modifiers,
            events: vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let output = context.run(input, |ctx| app.render(ctx));
        assert!(!output.shapes.is_empty());
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert!(app.is_reading());
        sender.send(Err("simulated read failure".into())).unwrap();
        app.poll_read();
        assert!(!app.is_reading());
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert_eq!(app.error.as_deref(), Some("simulated read failure"));
    }

    #[test]
    fn live_access_lock_never_starts_a_worker() {
        let (_, mut app) = app();
        app.start_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
        assert!(app.pending.is_none());
        assert!(app.status.contains("locked"));
        app.launch_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
    }

    fn recording_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn recording_frame_tall(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 1600.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn click_recording(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame(context, app, vec![]);
        let pos = app.recording_buttons[id].center();
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
    }

    #[test]
    fn offline_test_recording_window_does_not_start_a_device_or_a_worker() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("offline-test-take.wav");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..Default::default()
        });
        click_recording(&context, &mut app, "open-recording");
        assert!(app.recording_open);
        assert!(app.recording_visible.contains(recording::OFFLINE_BANNER));
        assert!(
            app.recording_visible
                .contains(recording::DEVICE_CAPTURE_LOCKED)
        );
        assert!(app.recording_visible.contains(recording::NO_AUTO_START));
        assert!(app.recording_visible.contains(recording::PHASE_IDLE));
        assert!(app.recording_visible.contains("not a live route change"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.recording_capture_attempts, 0);
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        click_recording(&context, &mut app, "start");
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert!(app.read_worker.is_none());
        click_recording(&context, &mut app, "route-clean");
        click_recording(&context, &mut app, "clock-44100");
        click_recording(&context, &mut app, "input-synthetic");
        assert!(app.recording_visible.contains("not a live route change"));
        assert!(!app.recorder.live_monitor_changed());
        assert!(!app.recorder.live_route_changed());
        click_recording(&context, &mut app, "start");
        assert_eq!(app.recorder.phase(), recording::Phase::Recording);
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
        assert!(!app.recorder.device_opened());
        click_recording(&context, &mut app, "feed");
        assert!(app.recording_visible.contains("Left peak 1000"));
        assert!(app.recording_visible.contains("Right peak 2000"));
        click_recording(&context, &mut app, "stop");
        click_recording(&context, &mut app, "save");
        let wav = std::fs::read(&path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
        let sidecar =
            std::fs::read_to_string(format!("{}.recording.json", path.display())).unwrap();
        assert!(sidecar.contains("\"device_recording\": false"));
        assert!(sidecar.contains("\"sample_rate_hz\": 44100"));
        assert!(!sidecar.contains("39062"));
        assert!(!sidecar.contains("78125"));
        assert!(app.read_worker.is_none());
        app.start_read();
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    fn prepare_quit_take(app: &mut Workbench, clock: ClockSelection) {
        app.recorder
            .select_route(RecordRoute::SendCleanGuitar)
            .unwrap();
        app.recorder.select_clock(clock).unwrap();
        app.recorder
            .select_input(recording::InputSelection::Synthetic)
            .unwrap();
        app.recorder.start().unwrap();
        app.recorder.feed().unwrap();
    }

    #[test]
    fn recording_quit_cancel_and_failed_save_retain_audio_until_success() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("blocked.wav");
        let good = dir.path().join("saved.wav");
        std::fs::write(&blocked, b"keep").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            save: [None, Some(blocked.clone()), Some(good.clone())].into(),
            ..Default::default()
        });
        prepare_quit_take(&mut app, ClockSelection::Integer(44_100));
        let levels = app.recorder.levels();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert_eq!(app.recorder.phase(), recording::Phase::Recording);
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // explicit stop, cancelled dialog
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.levels(), levels);
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save"); // refused overwrite
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
        assert_eq!(app.recorder.levels(), levels);
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save");
        assert!(!app.recorder.has_unsaved_take());
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        let decoded = recording::parse_wav(&std::fs::read(&good).unwrap()).unwrap();
        assert!(!decoded.data.is_empty());
        assert!(
            decoded
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .all(|frame| *frame == [0xe8, 3, 0x30, 0xf8])
        );
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn native_clock_quit_cancelled_save_retains_the_take_until_explicit_discard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.dialogs = Box::new(FakeDialogs {
            save: [None].into(),
            ..Default::default()
        });
        app.recorder.stop().unwrap();
        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // cancelled AIFF picker
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        assert!(app.recorder.has_unsaved_take());
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.recorder.device_opened());
    }

    #[test]
    fn native_recording_quit_saves_an_exact_clock_file_and_retains_its_source() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.aiff");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..Default::default()
        });
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        let bytes = std::fs::read(&path).unwrap();
        let source = recording::aiff::decode(&bytes).unwrap();
        assert_eq!(source.len(), 256 * 6);
        assert!(
            source
                .as_chunks::<6>()
                .0
                .iter()
                .all(|frame| *frame == [0xe8, 3, 0, 0x30, 0xf8, 0xff])
        );
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(recording::sidecar_path(&path).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["native_clock_exported"], true);
        assert_eq!(metadata["source_clock"]["denominator"], 2);
        app.permit_close = false;
        app.recorder.cancel().unwrap();
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(!app.permit_close);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(app.read_worker.is_none());
        assert_eq!(app.recording_capture_attempts, 0);
    }

    #[test]
    fn guitar_and_pod_quit_continuations_cannot_skip_an_unsaved_take() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let saved = dir.path().join("guitar.vwm");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        app.change(|preset| preset.rename("Guitar changed"));
        let index = app.pod_library.import_bytes(sample_pod_tone()).unwrap();
        app.pod_selected = Some(index);
        app.pod_name = "POD changed".into();
        app.apply_pod_name();
        prepare_quit_take(&mut app, ClockSelection::Integer(48_000));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        click_prompt(&context, &mut app, "save");
        assert!(saved.exists());
        assert!(!app.dirty());
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_library.any_dirty());
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        // Background editors cannot alter the take while this decision is pending.
        app.recording_open = true;
        click_recording(&context, &mut app, "cancel");
        assert!(app.recorder.has_unsaved_take());
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.recorder.has_unsaved_take());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(!app.recorder.has_unsaved_take());
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn load_recording_wav_refuses_a_nonregular_path_without_opening_it() {
        let (_context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-regular-wav");
        std::fs::create_dir(&path).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            wav: [Some(path)].into(),
            ..Default::default()
        });
        app.load_recording_wav();
        assert!(app.recorder.message().contains("regular file"));
        assert!(app.recorder.message().contains("not opened"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(!app.recorder.device_opened());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    fn click_pod(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_buttons[id].center();
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
    }

    fn click_pod_model_enable(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::PodModelEnable,
    ) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_model_enable_checks[&control].center();
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
    }

    fn open_pod_model_enable_menu(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        click_pod_model_enable_menu(context, app);
        recording_frame(context, app, vec![]);
    }

    fn click_pod_model_enable_menu(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_model_enable_menu.unwrap().center();
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
    }

    fn sample_pod_tone() -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
        let model = |slot: u32, model_id: u32| {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0, 0, 0, 1]);
            let mut body = b"MODL".to_vec();
            body.extend(chunk(b"MINF", &minf));
            body.extend(chunk(
                b"PARM",
                &[
                    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                ],
            ));
            chunk(b"LIST", &body)
        };
        let mut patch = b"PATC".to_vec();
        patch.extend(model(pod_preset::AMP_SLOT, 0x1111_0002));
        patch.extend(model(pod_preset::CAB_SLOT, 0x2222_0003));
        patch.extend(model(0x0003_00aa, 0x3333_0004));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_delay_note_tone(delay_note: u32, mod_note: u32) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        fn parameter(id: u32, kind: u32, bits: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(0x3f00_0000 | id).to_be_bytes());
            bytes.extend_from_slice(&kind.to_be_bytes());
            bytes.extend_from_slice(&bits.to_be_bytes());
            chunk(b"PARM", &bytes)
        }
        fn model(slot: u32, model_id: u32, parameters: &[Vec<u8>]) -> Vec<u8> {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0, 0, 0, 1]);
            let mut contents = b"MODL".to_vec();
            contents.extend(chunk(b"MINF", &minf));
            for parameter in parameters {
                contents.extend_from_slice(parameter);
            }
            chunk(b"LIST", &contents)
        }
        let mut info = vec![0; 80];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'D').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let system = model(
            0x0001_0000,
            0x3e70_0000,
            &[parameter(0x0020_0001, 1, 120.0_f32.to_bits())],
        );
        let modulation = model(
            pod_preset::MOD_SLOT,
            pod_preset::MOD_CONTROL_MODEL_IDS[0],
            &[
                parameter(0x0010_0000, 1, 0x3e80_0000),
                parameter(0x0020_0000, 0, mod_note),
            ],
        );
        let delay = model(
            pod_preset::DELAY_SLOT,
            0x0202_0002,
            &[
                parameter(0x0010_0000, 1, 0x3e80_0000),
                parameter(0x0020_0000, 0, delay_note),
            ],
        );
        let mut patch = b"PATC".to_vec();
        patch.extend(system);
        patch.extend(modulation);
        patch.extend(delay);
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_pod_bundle(tones: &[Vec<u8>]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut root = b"L6CO".to_vec();
        root.extend(chunk(b"JUNK", &[0x71, 0x72, 0x73]));
        for (index, tone) in tones.iter().enumerate() {
            let mut bank = if index == 0 {
                b"BANK".to_vec()
            } else {
                b"FLDR".to_vec()
            };
            let mut info = vec![0; 68];
            info[4..6].copy_from_slice(&u16::from(b'A' + index as u8).to_be_bytes());
            bank.extend(chunk(b"BINF", &info));
            bank.extend(chunk(b"NOTE", &[0x91, 0x92, 0x93]));
            bank.extend_from_slice(tone);
            root.extend(chunk(b"LIST", &bank));
        }
        chunk(b"FORM", &root)
    }

    fn sample_fixed_pod_bank(tones: &[Vec<u8>]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&u16::from(b'A').to_be_bytes());
        let mut bank = b"BANK".to_vec();
        bank.extend(chunk(b"BINF", &info));
        bank.extend(chunk(b"BEFR", &[0x11, 0x22, 0x33]));
        for (index, tone) in tones.iter().enumerate() {
            bank.extend_from_slice(tone);
            if index + 1 < tones.len() {
                bank.extend(chunk(b"MID!", &[0x44, 0x55, 0x66]));
            }
        }
        bank.extend(chunk(b"AFTR", &[0x77, 0x88, 0x99]));
        let mut root = b"L6BA".to_vec();
        root.extend(chunk(b"ROOT", &[0xaa, 0xbb, 0xcc]));
        root.extend(chunk(b"LIST", &bank));
        root.extend(chunk(b"TAIL", &[0xdd, 0xee, 0xff]));
        chunk(b"FORM", &root)
    }

    fn as_fixed_bank_tone(mut tone: Vec<u8>) -> Vec<u8> {
        tone[8..12].copy_from_slice(b"L6PA");
        tone
    }

    fn sample_drive_pod_tone() -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        fn parameter(id: u32, value: f32) -> Vec<u8> {
            let mut data = Vec::new();
            data.extend_from_slice(&((0x3fu32 << 24) | id).to_be_bytes());
            data.extend_from_slice(&1u32.to_be_bytes());
            data.extend_from_slice(&value.to_bits().to_be_bytes());
            chunk(b"PARM", &data)
        }
        fn model(slot: u32, model_id: u32, parameters: &[Vec<u8>]) -> Vec<u8> {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0, 0, 0, 1]);
            let mut body = b"MODL".to_vec();
            body.extend(chunk(b"MINF", &minf));
            for parameter in parameters {
                body.extend_from_slice(parameter);
            }
            body.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
            chunk(b"LIST", &body)
        }

        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut patch = b"PATC".to_vec();
        patch.extend(model(
            pod_preset::AMP_SLOT,
            pod_preset::PLEXI_45_MODEL,
            &[parameter(pod_preset::AMP_DRIVE_PARAM, 0.5)],
        ));
        patch.extend(model(
            pod_preset::STOMP_SLOT,
            pod_preset::STOMP_DRIVE_MODEL_ID,
            &[
                parameter(pod_preset::STOMP_DRIVE_PARAM, 0.25),
                parameter(pod_preset::STOMP_GAIN_PARAM, 0.5),
                parameter(pod_preset::STOMP_TONE_PARAM, 0.75),
            ],
        ));
        patch.extend(model(
            pod_preset::MOD_SLOT,
            pod_preset::MOD_CONTROL_MODEL_IDS[0],
            &[
                parameter(0x0010_0001, 0.2),
                parameter(0x0010_0002, 0.4),
                parameter(0x0010_0003, 0.6),
            ],
        ));
        patch.extend(model(
            pod_preset::REVERB_SLOT,
            pod_preset::REVERB_CONTROL_MODEL_IDS[0],
            &[parameter(0x0010_0000, 0.3), parameter(0x0010_0002, 0.6)],
        ));
        patch.extend(model(
            pod_preset::DELAY_SLOT,
            pod_preset::DELAY_CONTROL_MODEL_IDS[0],
            &[
                parameter(0x0010_0001, 0.2),
                parameter(0x0010_0002, 0.4),
                parameter(0x0010_0003, 0.6),
            ],
        ));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_cab_mic_pod_tone(cab_id: u32, mic_value: u32) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = id.to_vec();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                out.push(0xa5);
            }
            out
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'C').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut minf = Vec::new();
        minf.extend_from_slice(&cab_id.to_be_bytes());
        minf.extend_from_slice(&pod_preset::CAB_SLOT.to_be_bytes());
        minf.extend_from_slice(&[0x80, 0xa1, 0xb2, 0xfe]);
        let mut mic = Vec::new();
        mic.extend_from_slice(&0x3f00_0000u32.to_be_bytes());
        mic.extend_from_slice(&0u32.to_be_bytes());
        mic.extend_from_slice(&mic_value.to_be_bytes());
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &mic));
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_pod_eq_tone() -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut model = b"MODL".to_vec();
        let mut minf = Vec::new();
        minf.extend_from_slice(&pod_preset::PODXT_EQ_MODEL_ID.to_be_bytes());
        minf.extend_from_slice(&pod_preset::PODXT_EQ_SLOT.to_be_bytes());
        minf.extend_from_slice(&[0, 0, 0, 1]);
        model.extend(chunk(b"MINF", &minf));
        for id in 0x0010_0000u32..=0x0010_0007 {
            let mut parm = Vec::new();
            parm.extend_from_slice(&(0x3f00_0000 | id).to_be_bytes());
            parm.extend_from_slice(&1u32.to_be_bytes());
            parm.extend_from_slice(&0x3f00_0000u32.to_be_bytes());
            model.extend(chunk(b"PARM", &parm));
        }
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    #[test]
    fn pod_model_enable_gui_clicks_direct_tone_and_bundle_and_disables_noncanonical_state() {
        let (context, mut app) = app();
        let mut amp_tone = sample_pod_tone();
        let pinf_header = amp_tone
            .windows(4)
            .position(|window| window == b"PINF")
            .unwrap();
        let pinf_data = pinf_header + 8;
        amp_tone[pinf_data + 72..pinf_data + 76]
            .copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let original = amp_tone.clone();
        app.pod_library.import_bytes(amp_tone).unwrap();
        app.pod_selected = Some(0);
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Eq)
        );
        let mut marker = 0x1111_0002u32.to_be_bytes().to_vec();
        marker.extend_from_slice(&pod_preset::AMP_SLOT.to_be_bytes());
        let minf_body = original
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        let enabled_offset = minf_body + 11;
        let mut expected = original.clone();
        expected[enabled_offset] = 0;
        click_pod_model_enable(&context, &mut app, pod_preset::PodModelEnable::Amp);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), expected);
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        let mut short_minfo = original.clone();
        let minf_header = short_minfo
            .windows(4)
            .position(|window| window == b"MINF")
            .unwrap();
        short_minfo[minf_header + 4..minf_header + 8].copy_from_slice(&11u32.to_be_bytes());
        short_minfo[minf_header + 8 + 11] = 0;
        let short_original = short_minfo.clone();
        app.pod_library.clear();
        app.pod_library.import_bytes(short_minfo).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert!(
            app.pod_library
                .tone(0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Amp)
                .is_err()
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), short_original);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_model_enable_menu(&context, &mut app);

        let mut noncanonical = original;
        noncanonical[enabled_offset] = 0x80;
        let state = pod_preset::PodTone::parse(noncanonical.clone())
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Amp)
            .unwrap();
        assert_eq!(
            state,
            pod_preset::ModelEnableState::NonCanonical {
                raw: 0x80,
                source_interpretation: false,
            }
        );
        app.pod_library.clear();
        app.pod_library.import_bytes(noncanonical.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), noncanonical);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_model_enable_menu(&context, &mut app);

        let eq_tone = sample_pod_eq_tone();
        app.load_pod_bundle_bytes(sample_pod_bundle(std::slice::from_ref(&eq_tone)))
            .unwrap();
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Eq)
        );
        click_pod_model_enable(&context, &mut app, pod_preset::PodModelEnable::Eq);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
        assert!(app.pod_message.contains("Nothing was sent"));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }

        let dir = tempfile::tempdir().unwrap();
        let collision = dir.path().join("existing.vwb");
        std::fs::write(&collision, b"keep existing bytes").unwrap();
        app.dialogs.queue_pod_save(Some(collision.clone()));
        app.pending = Some(Pending::QuitPod);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());

        app.pending = Some(Pending::QuitPod);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some(), "failed save must keep quit pending");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");

        let saved = dir.path().join("saved.vwb");
        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap())
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
    }

    fn sample_pod_eq_tone_with_words(frequency: u32, gain: u32) -> Vec<u8> {
        let mut bytes = sample_pod_eq_tone();
        let marker = 0x3f00_0000u32.to_be_bytes();
        let mut offset = 0;
        let mut parameter = 0;
        while let Some(found) = bytes[offset..]
            .windows(4)
            .position(|window| window == marker)
        {
            let value_offset = offset + found;
            let value = if parameter % 2 == 0 { frequency } else { gain };
            bytes[value_offset..value_offset + 4].copy_from_slice(&value.to_be_bytes());
            offset = value_offset + 4;
            parameter += 1;
            if parameter == 8 {
                break;
            }
        }
        assert_eq!(parameter, 8);
        bytes
    }

    fn sample_stomp_wave_pod_tone() -> Vec<u8> {
        sample_stomp_wave_pod_tone_with_bits(0x3f00_0000)
    }

    fn sample_stomp_wave_pod_tone_with_bits(bits: u32) -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let mut model_slot = pod_preset::STOMP_DRIVE_MODEL_ID.to_be_bytes().to_vec();
        model_slot.extend_from_slice(&pod_preset::STOMP_SLOT.to_be_bytes());
        let offset = bytes
            .windows(model_slot.len())
            .position(|window| window == model_slot)
            .expect("synthetic Stomp MINF");
        bytes[offset..offset + 4]
            .copy_from_slice(&pod_preset::STOMP_SYNTH_WAVE_MODEL_IDS[0].to_be_bytes());
        let mut value_record = Vec::new();
        value_record.extend_from_slice(
            &((0x3fu32 << 24) | pod_preset::STOMP_SYNTH_WAVE_PARAM).to_be_bytes(),
        );
        value_record.extend_from_slice(&1u32.to_be_bytes());
        value_record.extend_from_slice(&0x3e80_0000u32.to_be_bytes());
        let value_offset = bytes
            .windows(value_record.len())
            .rposition(|window| window == value_record)
            .expect("synthetic Stomp wave PARM")
            + 8;
        bytes[value_offset..value_offset + 4].copy_from_slice(&bits.to_be_bytes());
        bytes
    }

    fn sample_stomp_harmony_pod_tone() -> Vec<u8> {
        sample_stomp_harmony_pod_tone_with_duplicate(None)
    }

    fn sample_stomp_harmony_pod_tone_with_duplicate(duplicate: Option<(u32, u8, u32)>) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = id.to_vec();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                out.push(0xa5);
            }
            out
        }
        let mut info = vec![0; 80];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut patch = b"PATC".to_vec();
        let mut model = b"MODL".to_vec();
        let mut minf = Vec::new();
        minf.extend_from_slice(&0x020a_0009u32.to_be_bytes());
        minf.extend_from_slice(&pod_preset::STOMP_SLOT.to_be_bytes());
        minf.extend_from_slice(&[1, 2, 3, 4]);
        model.extend(chunk(b"MINF", &minf));
        let mut records = vec![
            (0x0010_0001u32, 0x3fu8, 0x3f00_0000u32),
            (0x0010_0002, 0x3f, 0x3f40_8102),
        ];
        if let Some(duplicate) = duplicate {
            records.push(duplicate);
        }
        for (id, prefix, bits) in records {
            let mut parm = Vec::new();
            parm.extend_from_slice(&((u32::from(prefix) << 24) | id).to_be_bytes());
            parm.extend_from_slice(&1u32.to_be_bytes());
            parm.extend_from_slice(&bits.to_be_bytes());
            model.extend(chunk(b"PARM", &parm));
        }
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_whammy_pod_tone() -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let old = pod_preset::STOMP_DRIVE_MODEL_ID.to_be_bytes();
        let offset = bytes
            .windows(4)
            .position(|word| word == old)
            .expect("synthetic Stomp MINF");
        bytes[offset..offset + 4]
            .copy_from_slice(&pod_preset::STOMP_WHAMMY_INTERVAL_MODEL_ID.to_be_bytes());
        bytes
    }

    fn sample_delay_association_pod_tone(model_id: u32) -> Vec<u8> {
        let default_bits = if model_id == 0x0202_0005 {
            0.4f32.to_bits()
        } else {
            0.6f32.to_bits()
        };
        sample_delay_association_pod_tone_with_bits(model_id, default_bits)
    }

    fn sample_delay_association_pod_tone_with_bits(model_id: u32, value_bits: u32) -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let old = pod_preset::DELAY_CONTROL_MODEL_IDS[0].to_be_bytes();
        let offset = bytes.windows(4).position(|word| word == old).unwrap();
        bytes[offset..offset + 4].copy_from_slice(&model_id.to_be_bytes());
        let parameter_id = if model_id == 0x0202_0005 {
            0x0010_0002
        } else {
            0x0010_0003
        };
        let mut marker = ((0x3fu32 << 24) | parameter_id).to_be_bytes().to_vec();
        marker.extend_from_slice(&1u32.to_be_bytes());
        let value_offset = bytes
            .windows(marker.len())
            .rposition(|window| window == marker)
            .unwrap()
            + 8;
        bytes[value_offset..value_offset + 4].copy_from_slice(&value_bits.to_be_bytes());
        bytes
    }

    fn click_pod_at(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
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
    }

    fn choose_cab_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame(context, app, vec![]);
        let combo = app.pod_cab_combo.expect("CAB choice combo is rendered");
        click_pod_at(context, app, combo.center());
        recording_frame(context, app, vec![]);
        let option = app.pod_cab_options[&model_id].center();
        click_pod_at(context, app, option);
        recording_frame(context, app, vec![]);
    }

    #[test]
    fn cab_popup_choice_persists_until_apply_and_isolated_per_tone() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let first = sample_cab_mic_pod_tone(0x0102_0000, 1);
        let second = sample_cab_mic_pod_tone(0x0102_0001, 2);
        app.pod_library.import_bytes(first.clone()).unwrap();
        app.pod_library.import_bytes(second.clone()).unwrap();
        app.pod_selected = Some(0);

        recording_frame(&context, &mut app, vec![]);
        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), first);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_cab_pending,
            Some((
                0,
                pod_preset::PodCabModelChoice::try_from(0x0102_0002).unwrap()
            ))
        );
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0002);

        click_pod(&context, &mut app, "pod-cab-apply");
        let applied = app.pod_library.tone(0).unwrap();
        assert_eq!(applied.cab_model_id(), Some(0x0102_0002));
        assert_eq!(applied.mic_choice().unwrap().value(), 1);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);

        choose_cab_popup(&context, &mut app, 0x0102_0003);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0003);
        recording_frame(&context, &mut app, vec![]);
        let second_row = app.pod_rows[&1].center();
        click_pod_at(&context, &mut app, second_row);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_cab_pending.unwrap().0, 1);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0001);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn cab_mic_gui_apply_collision_save_reopen_revert_and_quit_cancel() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = sample_cab_mic_pod_tone(0x0102_0000, 1);
        app.pod_library.import_bytes(original).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);

        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_model_id(),
            Some(0x0102_0000)
        );
        click_pod(&context, &mut app, "pod-cab-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_model_id(),
            Some(0x0102_0002)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            1
        );
        assert!(app.pod_library.is_dirty(0));

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.tone");
        std::fs::write(&collision, b"preserve existing destination").unwrap();
        let saved = directory.path().join("cab-copy.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"preserve existing destination"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(reopened.cab_model_id(), Some(0x0102_0002));
        assert_eq!(reopened.mic_choice().unwrap().value(), 1);

        choose_cab_popup(&context, &mut app, 0x0102_0003);
        click_pod(&context, &mut app, "pod-cab-apply");
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_cab_pending, None);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0002);

        click_pod(&context, &mut app, "pod-mic-3");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            3
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    fn select_delay_heads_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayHeadsSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_delay_heads_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn select_delay_bits_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayBitsSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_delay_bits_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn select_stomp_wave_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompWaveSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_stomp_wave_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn stage_stomp_harmony1_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompHarmony1M335Selection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_harmony1_choices[&choice];
        click_pod_at_tall(context, app, rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn stage_delay_note_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayNoteChoice,
    ) {
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_delay_note_options[&choice];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_delay_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_delay_model_combo
            .expect("Delay model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_delay_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_mod_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_mod_model_combo
            .expect("Mod model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_mod_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn click_pod_button_tall(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame_tall(context, app, vec![]);
        let position = app.pod_buttons[id].center();
        click_pod_at_tall(context, app, position);
    }

    fn stage_mod_note_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::ModNoteChoice,
    ) {
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_mod_note_options[&choice];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn drag_delay_time_slider(context: &egui::Context, app: &mut Workbench, fraction: f32) {
        recording_frame_tall(context, app, vec![]);
        let rect = app
            .pod_delay_time_slider
            .expect("visible Delay TIME slider");
        let start = Pos2::new(rect.center().x, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * fraction, rect.center().y);
        recording_frame_tall(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame_tall(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_mod_speed_slider(context: &egui::Context, app: &mut Workbench, fraction: f32) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_mod_speed_slider.expect("visible Mod SPEED slider");
        let start = Pos2::new(rect.center().x, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * fraction, rect.center().y);
        recording_frame_tall(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame_tall(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn stage_stomp_harmony2_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompHarmony1457Selection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_harmony2_choices[&choice];
        click_pod_at_tall(context, app, rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn click_pod_at_tall(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
        recording_frame_tall(
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
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_drive_slider(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_drive_slider.expect("visible Amp Drive slider");
        let start = Pos2::new(rect.left() + rect.width() * 0.45, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.78, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_stomp_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::StompControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_stomp_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_whammy_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::StompWhammyKnob,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_whammy_knob_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_eq_frequency_slider(
        context: &egui::Context,
        app: &mut Workbench,
        band: pod_preset::PodxtEqBand,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_eq_frequency_sliders[&band];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.17, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_eq_gain_slider(
        context: &egui::Context,
        app: &mut Workbench,
        band: pod_preset::PodxtEqBand,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_eq_gain_sliders[&band];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.17, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_mod_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::ModControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_mod_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_reverb_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::ReverbControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_reverb_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_delay_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::DelayControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_delay_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    #[test]
    fn offline_pod_tone_window_does_not_open_a_device_or_edit_the_guitar() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let guitar = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("not-a-tone");
        std::fs::create_dir(&blocked).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(blocked)].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        assert!(app.pod_open);
        assert!(app.pod_visible.contains(pod_preset::OFFLINE_BANNER));
        assert!(app.pod_visible.contains(pod_preset::NOT_GUITAR));
        assert!(app.pod_visible.contains(pod_preset::NOT_SENT));
        assert!(app.pod_visible.contains("remain raw"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.pod_library.len(), 0);
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_message.contains("regular file"));
        assert!(app.pod_message.contains("not opened"));
        assert!(app.pod_library.is_empty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);

        let tone_path = directory.path().join("stage.tone");
        std::fs::write(&tone_path, sample_pod_tone()).unwrap();
        let guitar_path = directory.path().join("guitar.vwm");
        std::fs::write(&guitar_path, &guitar).unwrap();
        let saved = directory.path().join("copy.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(tone_path), Some(guitar_path)].into(),
            pod_save: [Some(saved.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_visible.contains("L6AS"));
        assert!(app.pod_visible.contains("11110002"));
        assert!(app.pod_visible.contains("22220003"));
        assert!(app.pod_visible.contains("remain raw"));
        assert!(
            app.pod_message.contains("Nothing is sent")
                || app.pod_visible.contains(pod_preset::NOT_SENT)
        );
        assert_eq!(app.pod_library.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        assert!(app.read_worker.is_none());
        app.pod_name = "Stage".into();
        click_pod(&context, &mut app, "apply-pod-name");
        assert!(app.pod_message.contains("Nothing was sent"));
        assert!(app.pod_visible.contains("Stage"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        click_pod(&context, &mut app, "save-pod");
        let written = std::fs::read(&saved).unwrap();
        let tone = pod_preset::PodTone::parse(written.clone()).unwrap();
        assert_eq!(tone.name(), "Stage");
        assert_eq!(tone.amp_model_id(), Some(0x1111_0002));
        assert_eq!(tone.cab_model_id(), Some(0x2222_0003));
        assert_eq!(tone.undecoded_slots(), 1);
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x11, 0x22, 0x33, 0x44])
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_message.starts_with("Saved a new copy"));
        assert_eq!(std::fs::read(&saved).unwrap(), written);
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_message.contains("Variax"));
        assert_eq!(app.pod_library.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        app.start_read();
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn pod_amp_model_apply_preserves_save_revert_and_quit_guards() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_drive_pod_tone();
        original[8..12].copy_from_slice(b"L6PA");
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_some());

        let no_amp = pod_preset::PodAmpModelChoice::NoAmp;
        click_pod(&context, &mut app, "pod-amp-no-amp");
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_drive_slider.is_none());
        assert!(app.pod_message.contains("Nothing was sent"));

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing.tone");
        std::fs::write(&collision, b"keep original").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), None].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep original");
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_message.contains("cancelled"));

        let saved = directory.path().join("saved.tone");
        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
        click_pod(&context, &mut app, "pod-amp-no-amp");
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );

        let clean = pod_preset::PodAmpModelChoice::Line6Clean;
        assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        click_pod(&context, &mut app, "pod-amp-line6-clean");
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(clean.model_id())
        );
        assert!(app.pod_library.is_dirty(0));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn offline_pod_bundle_window_navigates_edits_saves_and_guards_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let original_path = directory.path().join("source.vwb");
        let blocked_path = directory.path().join("occupied.vwb");
        let saved_path = directory.path().join("saved.vwb");
        let first = sample_drive_pod_tone();
        let second = sample_pod_tone();
        let original = sample_pod_bundle(&[first.clone(), second.clone()]);
        std::fs::write(&original_path, &original).unwrap();
        std::fs::write(&blocked_path, b"preserve existing").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(original_path.clone())].into(),
            pod_save: [Some(blocked_path.clone()), Some(saved_path.clone())].into(),
            ..Default::default()
        });

        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_bundle.as_ref().unwrap().bank_count(), 2);
        assert_eq!(app.pod_library.len(), 2);
        assert_eq!(app.pod_bundle.as_ref().unwrap().bank_name(1), Some("B"));
        let second_row = app.pod_rows[&1].center();
        click_pod_at(&context, &mut app, second_row);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);
        let first_row = app.pod_rows[&0].center();
        click_pod_at(&context, &mut app, first_row);
        drag_pod_drive_slider(&context, &mut app);
        let selected_value = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));

        click_pod(&context, &mut app, "save-pod");
        assert_eq!(std::fs::read(&blocked_path).unwrap(), b"preserve existing");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        click_pod(&context, &mut app, "save-pod");
        let saved = std::fs::read(&saved_path).unwrap();
        let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().amp_drive_fraction().unwrap(),
            selected_value
        );
        assert_eq!(reopened.tone(1, 0).unwrap().as_bytes(), second);
        assert_eq!(saved, app.pod_bundle.as_ref().unwrap().as_bytes());
        assert_eq!(std::fs::read(&original_path).unwrap(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());

        app.edit_pod_tone(0, |tone| {
            tone.set_amp_control_fraction(pod_preset::AmpControl::Drive, 0.2)
        })
        .unwrap();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn pod_membership_controls_are_folder_only_and_refresh_selection_and_pending_choices() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.l6c");
        let saved_path = directory.path().join("saved.l6c");
        let fixed_tone = sample_drive_pod_tone();
        let folder_tone = sample_pod_tone();
        let original = sample_pod_bundle(&[fixed_tone.clone(), folder_tone.clone()]);
        std::fs::write(&source_path, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved_path.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_bundle_bytes(original.clone()).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert!(!app.pod_buttons.contains_key("pod-duplicate-entry"));
        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-duplicate-entry"));

        app.pod_cab_pending = Some((
            1,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
        ));
        click_pod(&context, &mut app, "pod-duplicate-entry");
        let bundle = app.pod_bundle.as_ref().unwrap();
        assert_eq!(bundle.entry_count(), 3);
        assert_eq!(app.pod_library.len(), 3);
        assert_eq!(app.pod_selected, Some(2));
        assert_eq!(app.pod_library.tone(2).unwrap().as_bytes(), folder_tone);
        assert!(bundle.is_dirty());
        assert_ne!(
            app.pod_cab_pending,
            Some((
                2,
                pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
            ))
        );
        assert!(app.pod_cab_pending.is_none_or(|(index, _)| index != 1));
        assert_eq!(std::fs::read(&source_path).unwrap(), original);
        assert!(app.load_pod_bundle_bytes(original.clone()).is_err());
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 3);

        click_pod(&context, &mut app, "save-pod");
        let saved = std::fs::read(&saved_path).unwrap();
        let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
        assert_eq!(reopened.entry_count(), 3);
        assert_eq!(reopened.tone(1, 1).unwrap().as_bytes(), folder_tone);
        assert_eq!(saved, app.pod_bundle.as_ref().unwrap().as_bytes());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(std::fs::read(&source_path).unwrap(), original);

        click_pod(&context, &mut app, "pod-move-entry-up");
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());

        click_pod(&context, &mut app, "pod-remove-entry");
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 2);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_ne!(
            app.pod_cab_pending,
            Some((
                1,
                pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
            ))
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(folder_tone.clone())
                .unwrap()
                .name()
        );
        assert_eq!(std::fs::read(&source_path).unwrap(), original);
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn fixed_bank_reorder_controls_keep_selection_save_and_quit_state_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let first = as_fixed_bank_tone(sample_drive_pod_tone());
        let second = as_fixed_bank_tone(sample_pod_tone());
        let original = sample_fixed_pod_bank(&[first.clone(), second.clone()]);
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.vwb");
        let saved = directory.path().join("saved.vwb");
        std::fs::write(&collision, b"retain this destination").unwrap();

        app.pod_open = true;
        app.load_pod_bundle_bytes(original.clone()).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-move-entry-up"));
        assert!(app.pod_buttons.contains_key("pod-move-entry-down"));
        assert!(!app.pod_buttons.contains_key("pod-duplicate-entry"));
        assert!(!app.pod_buttons.contains_key("pod-remove-entry"));

        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        app.pod_cab_pending = Some((
            1,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
        ));
        let pending_before_rejected_action = app.pod_cab_pending;
        let original_bundle_bytes = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        let original_library_tones: Vec<_> = (0..app.pod_library.len())
            .map(|index| app.pod_library.tone(index).unwrap().as_bytes().to_vec())
            .collect();
        app.apply_pod_membership_action(PodMembershipAction::MoveDown, 99, 1);
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            original_bundle_bytes
        );
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_cab_pending, pending_before_rejected_action);
        assert_eq!(
            (0..app.pod_library.len())
                .map(|index| { app.pod_library.tone(index).unwrap().as_bytes().to_vec() })
                .collect::<Vec<_>>(),
            original_library_tones
        );

        click_pod(&context, &mut app, "pod-move-entry-up");
        let bundle = app.pod_bundle.as_ref().unwrap();
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), second);
        assert_eq!(bundle.tone(0, 1).unwrap().as_bytes(), first);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), second);
        assert_eq!(app.pod_selected, Some(0));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(second.clone()).unwrap().name()
        );
        assert!(bundle.is_dirty());
        assert!(app.pod_cab_pending.is_none());
        assert!(app.pod_message.contains("fixed bank"));
        assert!(std::fs::read(&collision).unwrap() == b"retain this destination");

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());

        app.request_close(&context);
        app.dialogs.queue_pod_save(Some(collision.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"retain this destination"
        );

        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            pod_preset::PodBundleLibrary::parse(std::fs::read(&saved).unwrap())
                .unwrap()
                .as_bytes(),
            app.pod_bundle.as_ref().unwrap().as_bytes()
        );

        click_pod(&context, &mut app, "pod-move-entry-down");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_selected, Some(1));
        app.permit_close = false;
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.pending.is_none());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            std::fs::read(&saved).unwrap()
        );
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_library.len(), 2);
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn reverting_pod_membership_before_recording_quit_cancel_keeps_selection_valid() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let fixed_tone = sample_drive_pod_tone();
        let folder_tone = sample_pod_tone();
        let original = sample_pod_bundle(&[fixed_tone, folder_tone.clone()]);
        app.pod_open = true;
        app.load_pod_bundle_bytes(original).unwrap();
        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        recording_frame(&context, &mut app, vec![]);

        click_pod(&context, &mut app, "pod-duplicate-entry");
        assert_eq!(app.pod_selected, Some(2));
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 3);

        prepare_quit_take(&mut app, ClockSelection::Integer(48_000));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 2);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(folder_tone).unwrap().name()
        );
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));

        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_rows.contains_key(&1));
        assert!(app.pod_buttons.contains_key("pod-duplicate-entry"));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn podxt_eq_gui_edits_preserve_maximum_frequency_and_gain_on_render() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("eq-endpoints.l6t");
        let original = sample_pod_eq_tone_with_words(0x3f80_0000, 0x3f80_0000);
        std::fs::write(&source, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_tone();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        let band = pod_preset::PodxtEqBand::One;
        drag_pod_eq_gain_slider(&context, &mut app, band);
        let (frequency, _) = app
            .pod_library
            .tone(0)
            .unwrap()
            .podxt_eq_band_raw_values(band)
            .unwrap();
        assert_eq!(frequency.to_bits(), 0x3f80_0000);
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        drag_pod_eq_frequency_slider(&context, &mut app, band);
        let (_, gain) = app
            .pod_library
            .tone(0)
            .unwrap()
            .podxt_eq_band_raw_values(band)
            .unwrap();
        assert_eq!(gain.to_bits(), 0x3f80_0000);
        assert!(app.pod_library.is_dirty(0));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn podxt_eq_graph_slider_edits_two_words_and_preserves_failed_save_state() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("eq.l6t");
        let collision = directory.path().join("occupied.l6t");
        let saved = directory.path().join("saved.l6t");
        let original = sample_pod_eq_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&collision, b"keep this file").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            pod_save: [None, Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_tone();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_eq_frequency_sliders.len(), 4);
        assert_eq!(app.pod_eq_gain_sliders.len(), 4);
        let before = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_pod_eq_frequency_slider(&context, &mut app, pod_preset::PodxtEqBand::One);
        assert!(app.pod_library.is_dirty(0));
        let after = app.pod_library.tone(0).unwrap().as_bytes();
        let changed: Vec<_> = before
            .iter()
            .zip(after)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert!(!changed.is_empty());
        assert!(app.pod_message.contains("Nothing was sent"));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(
            app.pod_library.is_dirty(0),
            "keep editing retains the applied EQ edit"
        );

        click_pod(&context, &mut app, "save-pod");
        assert!(
            app.pod_library.is_dirty(0),
            "cancel keeps the applied EQ edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep this file");
        assert!(
            app.pod_library.is_dirty(0),
            "collision keeps the applied EQ edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_ne!(reopened.as_bytes(), before);
    }

    fn click_pod_window_close(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let rect = context
            .memory(|memory| memory.area_rect(egui::Id::new("Offline PODxt Live tones")))
            .expect("POD window");
        let pos = Pos2::new(rect.right() - 20.0, rect.top() + 18.0);
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
    }

    fn pod_tone_name(app: &Workbench, index: usize) -> String {
        app.pod_library.tone(index).expect("tone").name().to_owned()
    }

    #[test]
    fn applied_pod_rename_blocks_quit_until_each_tone_is_saved_or_discarded() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let guitar = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let tone_a = directory.path().join("a.tone");
        let tone_b = directory.path().join("b.tone");
        std::fs::write(&tone_a, sample_pod_tone()).unwrap();
        std::fs::write(&tone_b, sample_pod_tone()).unwrap();
        let blocked = directory.path().join("blocked.tone");
        std::fs::write(&blocked, b"keep").unwrap();
        let alpha = directory.path().join("alpha.tone");
        let beta = directory.path().join("beta.tone");
        let guitar_copy = directory.path().join("guitar-copy.vwm");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(tone_a), Some(tone_b)].into(),
            pod_save: [
                None,
                Some(blocked.clone()),
                Some(alpha.clone()),
                Some(beta.clone()),
            ]
            .into(),
            save: [Some(guitar_copy.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        app.pod_name = "Typed only".into();
        app.request_close(&context);
        assert!(app.pending.is_none());
        assert!(!app.permit_close);
        assert!(!app.pod_library.any_dirty());
        app.pod_name = "Alpha".into();
        click_pod(&context, &mut app, "apply-pod-name");
        click_pod(&context, &mut app, "load-pod");
        app.pod_name = "Beta".into();
        click_pod(&context, &mut app, "apply-pod-name");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(!app.dirty());
        click_pod_window_close(&context, &mut app);
        assert!(!app.pod_open);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(!app.permit_close);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(app.pod_message.contains("cancelled"));
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(!app.pod_message.starts_with("Saved a new copy"));
        assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());

        app.change(|preset| preset.rename("Guitar edit"));
        assert!(app.dirty());
        let guitar_edited = app.document.as_ref().unwrap().as_bytes().to_vec();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.permit_close);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(!app.dirty());
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert_eq!(std::fs::read(&guitar_copy).unwrap(), guitar_edited);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));

        click_prompt(&context, &mut app, "save");
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        let saved_alpha = pod_preset::PodTone::parse(std::fs::read(&alpha).unwrap()).unwrap();
        assert_eq!(saved_alpha.name(), "Alpha");
        assert_eq!(saved_alpha.amp_model_id(), Some(0x1111_0002));
        assert!(!beta.exists());

        click_prompt(&context, &mut app, "save");
        assert!(!app.pod_library.any_dirty());
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        let saved_beta = pod_preset::PodTone::parse(std::fs::read(&beta).unwrap()).unwrap();
        assert_eq!(saved_beta.name(), "Beta");
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar_edited);

        app.permit_close = false;
        app.pod_selected = Some(0);
        app.pod_name = "Gamma".into();
        app.apply_pod_name();
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(!app.pod_library.any_dirty());
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert!(app.pod_message.contains("Discarded"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar_edited);
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn amp_stomp_mod_delay_and_reverb_sliders_edit_selected_tone_and_quit_guard_keeps_or_saves_it()
    {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        let directory = tempfile::tempdir().unwrap();
        let admitted_path = directory.path().join("admitted-drive.tone");
        let unavailable_path = directory.path().join("unavailable.tone");
        let blocked_path = directory.path().join("blocked-copy.tone");
        let saved_path = directory.path().join("drive-copy.tone");
        let admitted_bytes = sample_drive_pod_tone();
        let unavailable_bytes = sample_pod_tone();
        std::fs::write(&admitted_path, &admitted_bytes).unwrap();
        std::fs::write(&unavailable_path, &unavailable_bytes).unwrap();
        std::fs::write(&blocked_path, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(admitted_path), Some(unavailable_path)].into(),
            pod_save: [Some(blocked_path.clone()), Some(saved_path.clone())].into(),
            ..Default::default()
        });

        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_library.len(), 2, "{}", app.pod_message);
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_none());
        assert!(app.pod_stomp_sliders.is_empty());
        assert!(
            app.pod_library
                .tone(1)
                .unwrap()
                .amp_drive_fraction()
                .is_err()
        );
        assert!(!app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));

        let first_row = app.pod_rows[&0].center();
        click_pod_at(&context, &mut app, first_row);
        assert_eq!(app.pod_selected, Some(0));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_some());
        let stomp_controls = [
            pod_preset::StompControl::Drive,
            pod_preset::StompControl::Gain,
            pod_preset::StompControl::Tone,
        ];
        assert_eq!(app.pod_stomp_sliders.len(), stomp_controls.len());
        assert_eq!(app.pod_mod_sliders.len(), 3);
        assert_eq!(app.pod_reverb_sliders.len(), 2);
        assert_eq!(app.pod_delay_sliders.len(), 3);
        let original_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        let original_stomp: Vec<_> = stomp_controls
            .into_iter()
            .map(|control| {
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
            })
            .collect();
        drag_pod_drive_slider(&context, &mut app);
        for control in stomp_controls {
            drag_pod_stomp_slider(&context, &mut app, control);
        }
        drag_pod_mod_slider(&context, &mut app, pod_preset::ModControl::Depth);
        drag_pod_reverb_slider(&context, &mut app, pod_preset::ReverbControl::Dwell);
        drag_pod_delay_slider(&context, &mut app, pod_preset::DelayControl::Feedback);
        let edited_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        assert_ne!(edited_drive.to_bits(), original_drive.to_bits());
        assert!((0.0..=1.0).contains(&edited_drive));
        let edited_stomp: Vec<_> = stomp_controls
            .into_iter()
            .map(|control| {
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
            })
            .collect();
        for (edited, original) in edited_stomp.iter().zip(&original_stomp) {
            assert_ne!(edited.to_bits(), original.to_bits());
            assert!((0.0..=1.0).contains(edited));
        }
        let edited_mod = app
            .pod_library
            .tone(0)
            .unwrap()
            .mod_control_fraction(pod_preset::ModControl::Depth)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_mod));
        assert_ne!(edited_mod.to_bits(), 0.2f32.to_bits());
        let edited_reverb = app
            .pod_library
            .tone(0)
            .unwrap()
            .reverb_control_fraction(pod_preset::ReverbControl::Dwell)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_reverb));
        assert_ne!(edited_reverb.to_bits(), 0.3f32.to_bits());
        let edited_delay = app
            .pod_library
            .tone(0)
            .unwrap()
            .delay_control_fraction(pod_preset::DelayControl::Feedback)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_delay));
        assert_ne!(edited_delay.to_bits(), 0.2f32.to_bits());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            unavailable_bytes
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .amp_drive_fraction()
                .unwrap()
                .to_bits(),
            edited_drive.to_bits()
        );
        for (control, edited) in stomp_controls.into_iter().zip(&edited_stomp) {
            assert_eq!(
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
                    .to_bits(),
                edited.to_bits()
            );
        }

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            std::fs::read(&blocked_path).unwrap(),
            b"keep existing destination"
        );

        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.pod_library.any_dirty());
        let saved_bytes = std::fs::read(saved_path).unwrap();
        let reopened = pod_preset::PodTone::parse(saved_bytes).unwrap();
        assert_eq!(
            reopened.amp_drive_fraction().unwrap().to_bits(),
            edited_drive.to_bits()
        );
        for (control, edited) in stomp_controls.into_iter().zip(&edited_stomp) {
            assert_eq!(
                reopened.stomp_control_fraction(control).unwrap().to_bits(),
                edited.to_bits()
            );
        }
        assert_eq!(
            reopened
                .mod_control_fraction(pod_preset::ModControl::Depth)
                .unwrap()
                .to_bits(),
            edited_mod.to_bits()
        );
        assert_eq!(
            reopened
                .reverb_control_fraction(pod_preset::ReverbControl::Dwell)
                .unwrap()
                .to_bits(),
            edited_reverb.to_bits()
        );
        assert_eq!(
            reopened
                .delay_control_fraction(pod_preset::DelayControl::Feedback)
                .unwrap()
                .to_bits(),
            edited_delay.to_bits()
        );
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            unavailable_bytes
        );
        assert!(!app.document.as_ref().unwrap().as_bytes().is_empty());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn stomp_harmony_gui_stages_until_apply_and_keeps_the_other_selector_independent() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("existing-harmony-copy.l6t");
        let saved = directory.path().join("harmony-copy.l6t");
        std::fs::write(&blocked, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(blocked.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_stomp_harmony_pod_tone())
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_stomp_harmony_pod_tone())
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        stage_stomp_harmony1_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1M335Selection::MinusOctave,
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_harmony_pending,
            Some((
                0,
                pod_preset::StompHarmonyControl::Harmony1M335,
                PendingStompHarmony::Harmony1M335(
                    pod_preset::StompHarmony1M335Selection::MinusOctave
                )
            ))
        );
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_harmony_pending.is_none());
        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_stomp_harmony1_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1M335Selection::MinusOctave,
        );
        click_pod(&context, &mut app, "pod-harmony1-apply");
        let after_first = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(after_first, original);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_harmony_pending.is_none());
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1457_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fifth)
        );

        stage_stomp_harmony2_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1457Selection::Fourth,
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), after_first);
        click_pod(&context, &mut app, "pod-harmony2-apply");
        let final_bytes = app.pod_library.tone(0).unwrap().as_bytes();
        assert_ne!(final_bytes, after_first);
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1457_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fourth)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1m335_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(
                pod_preset::StompHarmony1M335Selection::MinusOctave
            )
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"keep existing destination"
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.stomp_harmony_1457_selection().unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fourth)
        );
        assert_eq!(
            reopened.stomp_harmony_1m335_selection().unwrap(),
            pod_preset::StompHarmonyValue::Known(
                pod_preset::StompHarmony1M335Selection::MinusOctave
            )
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn mod_note_gui_stages_per_tone_and_applies_only_on_request() {
        let (context, mut app) = app();
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_delay_note_tone(0, 0))
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_delay_note_tone(0, 0))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_mod_pending,
            Some((0, PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)))
        );

        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_mod_pending.is_none());
        let second_original = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-mod-discard");
        assert!(app.pod_mod_pending.is_none());
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second_original);

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Quarter)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mod_speed_hz()
                .unwrap()
                .to_bits(),
            2.0_f32.to_bits()
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_mod_pending.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn mod_speed_gui_stages_discards_applies_and_retains_failed_edits() {
        let (context, mut app) = app();
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_delay_note_tone(0, 6))
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_delay_note_tone(0, 99))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(matches!(
            app.pod_mod_pending,
            Some((0, PendingModEdit::SpeedHz(_)))
        ));
        click_pod(&context, &mut app, "pod-mod-discard");
        assert!(app.pod_mod_pending.is_none());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        drag_mod_speed_slider(&context, &mut app, 0.2);
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.tone(0).unwrap().as_bytes() != original);
        assert!(app.pod_mod_pending.is_none());

        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        let invalid_original = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            invalid_original
        );
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            invalid_original
        );
        assert!(!app.pod_library.is_dirty(1));
        assert!(matches!(
            app.pod_mod_pending,
            Some((1, PendingModEdit::SpeedHz(_)))
        ));

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        drag_mod_speed_slider(&context, &mut app, 0.3);
        assert!(app.pod_mod_pending.is_some());
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert!(app.pod_mod_pending.is_none());
        assert!(!app.pod_library.is_dirty(0));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn mod_speed_bundle_gui_saves_without_overwrite_and_reopens() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing-mod-bank.l6c");
        let saved = directory.path().join("mod-bank-copy.l6c");
        std::fs::write(&collision, b"preserve existing MOD destination").unwrap();
        let original_tone = sample_delay_note_tone(0, 6);
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&original_tone));
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_bundle_bytes(original_bundle.clone()).unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            original_bundle.as_slice()
        );
        click_pod(&context, &mut app, "pod-mod-apply");
        assert!(app.pod_dirty());
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );

        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"preserve existing MOD destination"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let saved_bytes = std::fs::read(&saved).unwrap();
        assert_ne!(saved_bytes, original_bundle);

        click_pod(&context, &mut app, "load-pod");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );
        assert!(!app.pod_dirty());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn delay_model_gui_stages_per_tone_applies_only_id_and_can_discard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_delay_model_pending.unwrap().1.model_id(),
            0x0202_0003
        );
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        let applied = app.pod_library.tone(index).unwrap();
        assert_eq!(
            applied
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        let changed: Vec<_> = original
            .iter()
            .zip(applied.as_bytes())
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(app.pod_delay_model_pending, None);
        assert_eq!(app.pod_delay_pending, None);

        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
        assert_eq!(app.pod_delay_model_pending, None);
        assert_eq!(
            app.pod_library
                .tone(index)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );

        // A staged selector is isolated to its tone and discarded on navigation.
        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        assert!(app.pod_delay_model_pending.is_some());
        let second_row = app.pod_rows[&1];
        click_pod_at_tall(&context, &mut app, second_row.center());
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_delay_model_pending, None);

        // A failed model Apply preserves prior parameter staging and source bytes.
        let mut invalid = sample_delay_note_tone(0, 0);
        let model_id = 0x0202_0002u32.to_be_bytes();
        let offset = invalid
            .windows(4)
            .position(|word| word == model_id)
            .unwrap();
        invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
        invalid[8..12].copy_from_slice(b"L6PA");
        app.pod_library.replace(1, invalid.clone()).unwrap();
        app.pod_delay_pending = Some((1, PendingDelayEdit::TimeMs(350.0)));
        app.pod_delay_model_pending = Some((
            1,
            pod_preset::PodDelayModelChoice::try_from(0x0202_0003).unwrap(),
        ));
        let failed_choice = app.pod_delay_model_pending.unwrap().1;
        assert!(app.apply_pod_delay_model_choice(1, failed_choice).is_err());
        assert_eq!(
            app.pod_delay_pending,
            Some((1, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), invalid);
        assert_eq!(app.pod_delay_model_pending, Some((1, failed_choice)));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn mod_model_gui_directly_replaces_each_gated_current_id_and_invalidates_note_stage() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let gated: [u32; 13] = [
            0x0203_0015,
            0x0203_0012,
            0x0203_001e,
            0x0203_0014,
            0x0203_001f,
            0x0203_0013,
            0x0203_0011,
            0x0203_001a,
            0x0203_001b,
            0x0203_0017,
            0x0203_0018,
            0x0203_0019,
            0x0203_001c,
        ];
        for current_id in gated {
            let mut original = sample_delay_note_tone(0, 0);
            original[8..12].copy_from_slice(b"L6PA");
            let old_id = 0x0203_0004u32.to_be_bytes();
            let offset = original
                .windows(4)
                .position(|word| word == old_id)
                .expect("fixture has a Mod model id");
            original[offset..offset + 4].copy_from_slice(&current_id.to_be_bytes());
            let index = app.pod_library.import_bytes(original.clone()).unwrap();
            app.pod_selected = Some(index);
            recording_frame_tall(&context, &mut app, vec![]);
            stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
            choose_mod_model_popup(&context, &mut app, 0x0203_0005);
            assert_eq!(app.pod_mod_model_pending.unwrap().1.model_id(), 0x0203_0005);
            click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
            let applied = app.pod_library.tone(index).unwrap();
            let changed: Vec<_> = original
                .iter()
                .zip(applied.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            assert!(!changed.is_empty());
            assert!(
                changed
                    .iter()
                    .all(|byte| (offset..offset + 4).contains(byte))
            );
            assert_eq!(app.pod_mod_pending, None);
            assert_eq!(app.pod_mod_model_pending, None);
        }
    }

    #[test]
    fn mod_model_gui_applies_and_discards_within_the_selected_bundle_entry() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let old = 0x0203_0004u32.to_be_bytes();
        let offset = original
            .windows(4)
            .position(|word| word == old)
            .expect("fixture has a Mod model id");
        original[offset..offset + 4].copy_from_slice(&0x0203_0015u32.to_be_bytes());
        app.load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
            .unwrap();

        recording_frame_tall(&context, &mut app, vec![]);
        choose_mod_model_popup(&context, &mut app, 0x0203_0005);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-discard");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());

        choose_mod_model_popup(&context, &mut app, 0x0203_0005);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::MOD_SLOT)
                .unwrap()
                .model_id,
            0x0203_0005
        );
    }

    #[test]
    fn mod_model_same_id_and_rejected_apply_preserve_staged_note() {
        let (_context, mut app) = app();
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_mod_pending = Some((
            index,
            PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
        ));
        let same = pod_preset::PodModModelChoice::try_from(0x0203_0004).unwrap();
        app.apply_pod_mod_model_choice(index, same).unwrap();
        assert_eq!(
            app.pod_mod_pending,
            Some((
                index,
                PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)
            ))
        );
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);

        let mut invalid = original;
        let current = 0x0203_0004u32.to_be_bytes();
        let offset = invalid.windows(4).position(|word| word == current).unwrap();
        invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
        app.pod_library.replace(index, invalid.clone()).unwrap();
        let pending = Some((
            index,
            PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
        ));
        app.pod_mod_pending = pending;
        assert!(app.apply_pod_mod_model_choice(index, same).is_err());
        assert_eq!(app.pod_mod_pending, pending);
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), invalid);
    }

    #[test]
    fn delay_model_gui_can_directly_replace_each_pack_gated_current_model() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;

        for (ordinal, current_id) in (0x0202_000bu32..=0x0202_000fu32).enumerate() {
            let mut original = sample_delay_note_tone(0, 0);
            original[8..12].copy_from_slice(b"L6PA");
            let old_id = 0x0202_0002u32.to_be_bytes();
            let old_id_offset = original
                .windows(4)
                .position(|word| word == old_id)
                .expect("fixture has a Delay model id");
            original[old_id_offset..old_id_offset + 4].copy_from_slice(&current_id.to_be_bytes());
            let index = app.pod_library.import_bytes(original.clone()).unwrap();
            app.pod_selected = Some(index);

            recording_frame_tall(&context, &mut app, vec![]);
            stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
            assert!(app.pod_delay_pending.is_some());
            choose_delay_model_popup(&context, &mut app, 0x0202_0002);
            assert_eq!(
                app.pod_delay_model_pending.unwrap().1.model_id(),
                0x0202_0002,
                "pack-gated current model {current_id:08x} must allow direct first-choice staging"
            );

            if ordinal == 0 {
                click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
                assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
                assert!(app.pod_delay_pending.is_some());
                choose_delay_model_popup(&context, &mut app, 0x0202_0002);
            }

            click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
            let applied = app.pod_library.tone(index).unwrap();
            let changed: Vec<_> = original
                .iter()
                .zip(applied.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            let id_offset = original
                .windows(4)
                .position(|word| word == current_id.to_be_bytes())
                .unwrap();
            assert!(!changed.is_empty());
            assert!(
                changed
                    .iter()
                    .all(|offset| (id_offset..id_offset + 4).contains(offset)),
                "only the Delay MINF model-id word may change"
            );
            assert_eq!(
                applied
                    .models()
                    .iter()
                    .find(|model| model.slot == pod_preset::DELAY_SLOT)
                    .unwrap()
                    .model_id,
                0x0202_0002
            );
            assert_eq!(app.pod_delay_pending, None);
            assert_eq!(app.pod_delay_model_pending, None);
        }

        // A same-ID apply leaves a fresh NOTE/TIME stage available.
        let same_id = app.pod_selected.unwrap();
        app.pod_delay_pending = Some((same_id, PendingDelayEdit::TimeMs(350.0)));
        let same_choice = pod_preset::PodDelayModelChoice::try_from(0x0202_0002).unwrap();
        app.apply_pod_delay_model_choice(same_id, same_choice)
            .unwrap();
        assert_eq!(
            app.pod_delay_pending,
            Some((same_id, PendingDelayEdit::TimeMs(350.0)))
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn delay_model_apply_invalidates_stale_parameter_stage_in_bundle() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        app.load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert_eq!(app.pod_delay_pending, None);
        let (bank, preset) = app.pod_bundle.as_ref().unwrap().entry_location(0).unwrap();
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(bank, preset)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn delay_model_apply_save_collision_reopen_revert_and_quit_guard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.tone");
        let saved = directory.path().join("delay-model-copy.tone");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        app.pod_library.import_bytes(original).unwrap();
        app.pod_selected = Some(0);
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        recording_frame_tall(&context, &mut app, vec![]);
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert!(app.pod_dirty());
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let saved_tone = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            saved_tone
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );

        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert!(app.pod_dirty());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_dirty());
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert!(!app.pod_dirty());
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        assert!(!app.pod_dirty());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn delay_note_gui_stages_per_tone_applies_offline_and_keeps_unsaved_guards() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("delay-note-copy.l6c");
        let blocked = directory.path().join("occupied-delay-copy.l6c");
        let saved_time = directory.path().join("delay-time-copy.l6c");
        let blocked_time = directory.path().join("occupied-delay-time-copy.l6c");
        std::fs::write(&blocked, b"preserve existing NOTE destination").unwrap();
        std::fs::write(&blocked_time, b"preserve existing TIME destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                Some(blocked.clone()),
                Some(saved.clone()),
                Some(blocked_time.clone()),
                Some(saved_time.clone()),
            ]
            .into(),
            ..Default::default()
        });
        let original = sample_delay_note_tone(0, 0);
        app.pod_open = true;
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_delay_pending,
            Some((
                0,
                PendingDelayEdit::Note(pod_preset::DelayNoteChoice::Quarter)
            ))
        );
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_delay_pending.is_none());

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-delay-apply");
        let applied = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(applied, original);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_delay_pending.is_none());
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
        );
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"preserve existing NOTE destination"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            reopened.delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
        );
        assert!(!app.pod_library.is_dirty(0));

        let saved_note = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_delay_time_slider(&context, &mut app, 0.75);
        let PendingDelayEdit::TimeMs(staged_time) = app.pod_delay_pending.unwrap().1 else {
            panic!("TIME slider should stage a TIME edit");
        };
        assert!(staged_time > app.pod_library.tone(0).unwrap().delay_time_ms().unwrap());
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.pod_delay_pending,
            Some((0, PendingDelayEdit::TimeMs(staged_time))),
            "TIME staging must persist across frames"
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), saved_note);
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(
            app.pod_delay_pending.is_none(),
            "tone switch clears staged TIME"
        );
        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);

        drag_delay_time_slider(&context, &mut app, 0.75);
        click_pod(&context, &mut app, "pod-delay-discard");
        assert!(app.pod_delay_pending.is_none());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), saved_note);
        assert!(!app.pod_library.is_dirty(0));

        drag_delay_time_slider(&context, &mut app, 0.75);
        click_pod(&context, &mut app, "pod-delay-apply");
        let time_applied = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(time_applied, saved_note);
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off),
            "positive NOTE clears only when TIME Apply succeeds"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked_time).unwrap(),
            b"preserve existing TIME destination"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        let reopened_time =
            pod_preset::PodTone::parse(std::fs::read(&saved_time).unwrap()).unwrap();
        assert_eq!(reopened_time.as_bytes(), time_applied);
        assert!(!app.pod_library.is_dirty(0));

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-delay-apply");
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), time_applied);
        assert!(!app.pod_library.is_dirty(0));
    }

    #[test]
    fn delay_note_gui_rejects_coupled_mod_state_without_mutating() {
        let (context, mut app) = app();
        app.pod_open = true;
        let original = sample_delay_note_tone(0, 6);
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        click_pod(&context, &mut app, "pod-delay-apply");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_message.contains("Mod NOTE Off"));
        assert_eq!(
            app.pod_delay_pending,
            Some((
                0,
                PendingDelayEdit::Note(pod_preset::DelayNoteChoice::Quarter)
            ))
        );
    }

    #[test]
    fn delay_time_gui_bundle_apply_reverts_through_shared_bundle_path() {
        let (context, mut app) = app();
        let original = sample_delay_note_tone(6, 0);
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&original));
        app.pod_open = true;
        app.load_pod_bundle_bytes(original_bundle.clone()).unwrap();
        recording_frame_tall(&context, &mut app, vec![]);

        drag_delay_time_slider(&context, &mut app, 0.75);
        let pending = app.pod_delay_pending;
        assert!(matches!(pending, Some((0, PendingDelayEdit::TimeMs(_)))));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        click_pod(&context, &mut app, "pod-delay-apply");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off)
        );
        assert!(app.pod_delay_pending.is_none());

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(
            app.pod_delay_pending.is_none(),
            "bundle revert clears pending NOTE"
        );
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), original_bundle);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn stomp_harmony_gui_hides_a_selector_with_duplicate_parameter_records() {
        for target in [0x0010_0001u32, 0x0010_0002] {
            let (context, mut app) = app();
            app.pod_open = true;
            let duplicate_tone =
                sample_stomp_harmony_pod_tone_with_duplicate(Some((target, 0x3f, 0x3f00_0000)));
            let index = app.pod_library.import_bytes(duplicate_tone).unwrap();
            app.pod_selected = Some(index);
            let original = app.pod_library.tone(index).unwrap().as_bytes().to_vec();
            recording_frame_tall(&context, &mut app, vec![]);
            if target == 0x0010_0001 {
                assert!(app.pod_harmony1_choices.is_empty());
                assert!(!app.pod_buttons.contains_key("pod-harmony1-apply"));
                assert_eq!(app.pod_harmony2_choices.len(), 9);
            } else {
                assert!(app.pod_harmony2_choices.is_empty());
                assert!(!app.pod_buttons.contains_key("pod-harmony2-apply"));
                assert_eq!(app.pod_harmony1_choices.len(), 9);
            }
            assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(index));
            const {
                assert!(LIVE_ACCESS_LOCKED);
            }
        }
    }

    #[test]
    fn stomp_wave_choices_edit_dirty_save_quit_and_reopen_in_gui() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("stomp-wave-source.l6t");
        let blocked = directory.path().join("existing-copy.l6t");
        let saved = directory.path().join("stomp-wave-copy.l6t");
        let original = sample_stomp_wave_pod_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&blocked, b"preserve existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(blocked.clone()), None, Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_library.len(), 1, "{}", app.pod_message);
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(!app.pod_library.is_dirty(0));
        assert!(matches!(
            app.pod_library.tone(0).unwrap().stomp_wave_selection(),
            Ok(pod_preset::StompWaveValue::Known(
                pod_preset::StompWaveSelection::Wave5
            ))
        ));

        select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave1);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_wave_selection()
                .unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"preserve existing destination"
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);

        // A cancelled save leaves the dirty edit available for another choice/save.
        click_prompt(&context, &mut app, "save");
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert!(!app.pod_library.any_dirty());
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.stomp_wave_selection().unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn stomp_wave_gui_refuses_nonfinite_values_without_dirtying_the_library() {
        for raw_bits in [
            0x7fc0_1234,
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
        ] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("nonfinite-stomp-wave.l6t");
            let original = sample_stomp_wave_pod_tone_with_bits(raw_bits);
            std::fs::write(&source, &original).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));
            assert!(matches!(
                app.pod_library.tone(0).unwrap().stomp_wave_selection(),
                Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(matches!(
                app.pod_library.tone(0).unwrap().stomp_wave_selection(),
                Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            assert!(!app.pod_library.is_dirty(0));
            assert!(app.pod_message.contains("non-finite"));
            const {
                assert!(LIVE_ACCESS_LOCKED);
            }
        }
    }

    #[test]
    fn whammy_knob_gui_edit_preserves_other_control_and_guards_save_and_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("whammy-source.l6t");
        let blocked = directory.path().join("existing-copy.l6t");
        let saved = directory.path().join("whammy-copy.l6t");
        let original = sample_whammy_pod_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&blocked, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            pod_save: [Some(blocked.clone()), None, Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_whammy_knob_sliders.len(), 2);
        assert!(
            !app.pod_library.is_dirty(0),
            "opening and rendering preserve raw values"
        );
        let toe_before = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
            .unwrap()
            .to_bits();
        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Heel);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe_before
        );
        let edited_heel = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
            .unwrap()
            .to_bits();
        assert_ne!(edited_heel, 0x3e80_0000);
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"keep existing destination"
        );
        assert!(
            app.pod_library.is_dirty(0),
            "collision preserves dirty edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(
            app.pod_library.is_dirty(0),
            "cancelled save preserves dirty edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            edited_heel
        );
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe_before
        );

        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Toe);
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(!app.permit_close);
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn stomp_wave_bundle_gui_edit_is_dirty_and_saves_a_preserving_copy() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("stomp-wave-bank.vwb");
        let saved = directory.path().join("stomp-wave-bank-copy.vwb");
        let tone = sample_stomp_wave_pod_tone();
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&tone));
        std::fs::write(&source, &original_bundle).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_bundle.is_some());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        recording_frame_tall(&context, &mut app, vec![]);
        select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .stomp_wave_selection()
                .unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(std::fs::read(source).unwrap(), original_bundle);
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().stomp_wave_selection().unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn whammy_bundle_gui_routes_typed_knob_edit_through_collection_save() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("whammy-bank.vwb");
        let saved = directory.path().join("whammy-bank-copy.vwb");
        let tone = sample_whammy_pod_tone();
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&tone));
        std::fs::write(&source, &original_bundle).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_bundle.is_some());
        recording_frame_tall(&context, &mut app, vec![]);
        let heel = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
            .unwrap()
            .to_bits();
        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Toe);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        let toe = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
            .unwrap()
            .to_bits();
        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(std::fs::read(source).unwrap(), original_bundle);
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(0, pod_preset::StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            heel
        );
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(0, pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe
        );
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn delay_association_combos_keep_unknown_values_until_explicit_choice_and_save_reopen() {
        for model_id in [0x0202_0005, 0x0202_000d] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("association-source.l6t");
            let saved = directory.path().join("association-copy.l6t");
            std::fs::write(&source, sample_delay_association_pod_tone(model_id)).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                pod_save: [Some(saved.clone())].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.len(), 1, "{}", app.pod_message);
            assert_eq!(app.pod_selected, Some(0));
            recording_frame(&context, &mut app, vec![]);
            assert!(!app.pod_library.is_dirty(0));

            if model_id == 0x0202_0005 {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_heads_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { .. })
                ));
                select_delay_heads_choice(
                    &context,
                    &mut app,
                    pod_preset::DelayHeadsSelection::OneTwoThreeFour,
                );
                assert_eq!(
                    app.pod_library
                        .tone(0)
                        .unwrap()
                        .delay_heads_selection()
                        .unwrap(),
                    pod_preset::DelayAssociationValue::Known(
                        pod_preset::DelayHeadsSelection::OneTwoThreeFour
                    )
                );
            } else {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_bits_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { .. })
                ));
                select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
                assert_eq!(
                    app.pod_library
                        .tone(0)
                        .unwrap()
                        .delay_bits_selection()
                        .unwrap(),
                    pod_preset::DelayAssociationValue::Known(pod_preset::DelayBitsSelection::Four)
                );
            }
            assert!(app.pod_library.is_dirty(0));
            app.request_close(&context);
            assert!(matches!(app.pending, Some(Pending::QuitPod)));
            click_prompt(&context, &mut app, "save");
            assert!(app.permit_close);
            assert!(app.pending.is_none());
            let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
            if model_id == 0x0202_0005 {
                assert_eq!(
                    reopened.delay_heads_selection().unwrap(),
                    pod_preset::DelayAssociationValue::Known(
                        pod_preset::DelayHeadsSelection::OneTwoThreeFour
                    )
                );
            } else {
                assert_eq!(
                    reopened.delay_bits_selection().unwrap(),
                    pod_preset::DelayAssociationValue::Known(pod_preset::DelayBitsSelection::Four)
                );
            }
            const {
                assert!(LIVE_ACCESS_LOCKED);
            }
        }
    }

    #[test]
    fn delay_association_gui_refuses_nonfinite_values_without_dirtying_the_library() {
        for (model_id, raw_bits) in [
            (0x0202_0005, 0x7fc0_0001),
            (0x0202_0005, 0x7f80_0000),
            (0x0202_0005, 0xff80_0000),
            (0x0202_000d, 0x7fc0_0001),
            (0x0202_000d, 0x7f80_0000),
            (0x0202_000d, 0xff80_0000),
        ] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("nonfinite-association.l6t");
            let original = sample_delay_association_pod_tone_with_bits(model_id, raw_bits);
            std::fs::write(&source, &original).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));

            if model_id == 0x0202_0005 {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_heads_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
                ));
                select_delay_heads_choice(
                    &context,
                    &mut app,
                    pod_preset::DelayHeadsSelection::OneTwoThreeFour,
                );
            } else {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_bits_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
                ));
                select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
            }

            assert!(
                app.pod_message.contains("non-finite"),
                "{}",
                app.pod_message
            );
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));
            const {
                assert!(LIVE_ACCESS_LOCKED);
            }
        }
    }

    #[test]
    fn rendering_imported_out_of_range_values_does_not_edit_or_create_history() {
        let (context, mut app) = app();
        let mut bytes = include_bytes!("../tests/fixtures/synthetic-host-preset.vwm").to_vec();
        for (id, kind, old, new) in [
            (0x3f200000u32, 1u32, 6f32.to_bits(), f32::NAN.to_bits()),
            (0x3f200001, 1, 45f32.to_bits(), 95f32.to_bits()),
            (0x3f200000, 0, (-2i32) as u32, 1000),
            (0x3f200006, 1, 50f32.to_bits(), 200f32.to_bits()),
        ] {
            let pattern = [id.to_be_bytes(), kind.to_be_bytes(), old.to_be_bytes()].concat();
            let offset = bytes
                .windows(pattern.len())
                .position(|w| w == pattern)
                .unwrap();
            bytes[offset + 8..offset + 12].copy_from_slice(&new.to_be_bytes());
        }
        app.document = Some(EditorDocument::from_preset(
            PresetFile::parse(bytes.clone()).unwrap(),
            "imported.vwm".into(),
        ));
        for tab in [Tab::Body, Tab::Pickups, Tab::Tuning, Tab::Controls] {
            app.tab = tab;
            for _ in 0..2 {
                let _ = context.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.controls(ui));
                });
            }
            let doc = app.document.as_ref().unwrap();
            assert_eq!(doc.preset().as_bytes(), bytes);
            assert!(!doc.is_dirty());
            assert!(!doc.can_undo());
        }
    }

    #[test]
    fn missing_artwork_is_cached_without_altering_the_document() {
        let (context, mut app) = app();
        let before = app.document.as_ref().unwrap().preset().as_bytes().to_vec();
        let path = "definitely-not-an-original-asset.png";
        assert!(app.texture(&context, path).is_none());
        assert!(app.missing_textures.contains(path));
        assert!(app.texture(&context, path).is_none());
        assert_eq!(app.missing_textures.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert!(app.error.is_none());
    }

    #[test]
    fn invalid_arguments_return_errors_instead_of_panicking() {
        let root = Path::new("project");
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
            ]
            .into_iter()
            .map(str::to_owned),
            root,
        )
        .unwrap();
        assert_eq!(options.tab, Tab::Controls);
        assert_eq!(options.preset.as_deref(), Some(Path::new("model.vwm")));
    }

    #[test]
    fn fresh_install_without_assets_or_default_preset_can_open_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let options = parse_options(Vec::new(), directory.path()).unwrap();
        assert!(options.preset.is_none());
        let context = egui::Context::default();
        let mut app = Workbench::with_context(&context, options.preset, options.artwork);
        app.theme_polled = Instant::now();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.error.is_none());
        assert!(app.document.is_none());
        assert!(app.read_worker.is_none());
        assert!(app.textures.is_empty());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm"),
            )]
            .into(),
            ..Default::default()
        });
        app.open_dialog();
        assert!(app.document.is_some());
        assert!(app.error.is_none());
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.error.is_none());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.dirty());
    }

    #[test]
    fn desktop_file_argument_and_explicit_artwork_override_data_directory() {
        let root = Path::new("user-data");
        let parse = |args: &[&str]| parse_options(args.iter().map(|s| (*s).to_owned()), root);
        let options = parse(&["a preset.vwm", "--data-dir", "chosen"]).unwrap();
        assert_eq!(options.preset.as_deref(), Some(Path::new("a preset.vwm")));
        assert_eq!(options.artwork, Path::new("chosen/artwork"));
        for args in [
            vec!["--artwork", "images", "--data-dir", "chosen"],
            vec!["--data-dir", "chosen", "--artwork", "images"],
        ] {
            assert_eq!(parse(&args).unwrap().artwork, Path::new("images"));
        }
        assert!(parse(&["one.vwm", "two.vwm"]).is_err());
        assert!(parse(&["one.vwm", "--preset", "two.vwm"]).is_err());
        assert!(parse(&["--data-dir"]).is_err());
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

    #[test]
    fn bank_browser_clicks_preserve_edits_and_active_operation_blocks_browsing() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 900.))),
                    events,
                    ..Default::default()
                },
                |ctx| app.bank_browser(ctx),
            );
        }
        fn click(context: &egui::Context, app: &mut Workbench, row: (usize, usize)) {
            let pos = app.bank_rows[&row].center();
            frame(
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
            frame(
                context,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
        }
        for name in ["synthetic-bank.vwb", "synthetic-collection.l6c"] {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.);
            app.load(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(name),
            );
            frame(&context, &mut app, vec![]);
            assert_eq!(app.bank_rows.len(), 5);
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((1, 0)));
            click(&context, &mut app, (2, 1));
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((2, 1)));
            app.change(|p| p.rename("Edited bank preset"));
            click(&context, &mut app, (1, 0));
            let doc = app.document.as_ref().unwrap();
            assert_eq!(doc.selection(), Some((1, 0)));
            assert_eq!(
                doc.banks().unwrap()[2].presets()[1].name(),
                "Edited bank preset"
            );
            assert!(doc.is_dirty());
            let (sender, receiver) = std::sync::mpsc::channel();
            app.read_receiver = Some(receiver);
            click(&context, &mut app, (2, 1));
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((1, 0)));
            sender
                .send(Err("offline simulated failure".into()))
                .unwrap();
            app.poll_read();
            app.undo();
            assert!(!app.document.as_ref().unwrap().is_dirty());
        }
    }
}

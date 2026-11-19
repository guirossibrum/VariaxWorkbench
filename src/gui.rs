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
use variax_workbench::pod_preset::{self, Library, PodPresetFile};
use variax_workbench::recording::{self, ClockSelection};
use variax_workbench::rig::{HdEditorLibrary, ProductProfile, Rejection, RoleKind, Session};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType9Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    choice: pod_preset::HdType9Choice,
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
    hd_library: HdEditorLibrary,
    pod_selected: Option<usize>,
    hd_selected: Option<usize>,
    hd_effect_slot: usize,
    hd_default_target: pod_preset::HdDistortionDefault,
    hd_control: pod_preset::HdType1Control,
    hd_mapped_control: pod_preset::HdType1MappedControl,
    hd_control_percent: u8,
    hd_numeric_control: pod_preset::HdNumericControl,
    hd_numeric_value: i32,
    hd_type9_pending: Option<PendingHdType9Choice>,
    hd_setlist_index: usize,
    hd_preset_index: usize,
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
    pod_scroll_area: Option<(egui::Id, Rect, f32)>,
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
            hd_library: Session::initial_rig().new_hd_editor_library(),
            pod_selected: None,
            hd_selected: None,
            hd_effect_slot: 0,
            hd_default_target: pod_preset::HdDistortionDefault::JetFuzz,
            hd_control: pod_preset::HdType1Control::Screamer(pod_preset::ScreamerControl::Drive),
            hd_mapped_control: pod_preset::HD_TYPE1_MAPPED_CONTROLS[0],
            hd_control_percent: 0,
            hd_numeric_control: pod_preset::HD_NUMERIC_CONTROLS[0],
            hd_numeric_value: 0,
            hd_type9_pending: None,
            hd_setlist_index: 0,
            hd_preset_index: 0,
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
            pod_scroll_area: None,
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
                if self.pod_library.any_dirty()
                    || self.hd_library.any_dirty()
                    || self.hd_type9_pending.is_some()
                {
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
        } else if (self.pod_library.any_dirty()
            || self.hd_library.any_dirty()
            || self.hd_type9_pending.is_some())
            && !self.permit_close
        {
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
        let Some(index) = self.pod_library.first_dirty() else {
            if self.hd_library.any_dirty() || self.hd_type9_pending.is_some() {
                self.hd_quit_dialog(ctx);
            } else {
                self.continue_pending(ctx);
            }
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
            if self.save_next_dirty_pod()
                && !self.pod_library.any_dirty()
                && !self.hd_library.any_dirty()
            {
                self.continue_pending(ctx);
            }
        } else if discard
            && self.discard_next_dirty_pod()
            && !self.pod_library.any_dirty()
            && !self.hd_library.any_dirty()
        {
            self.continue_pending(ctx);
        }
    }

    fn hd_quit_dialog(&mut self, ctx: &egui::Context) {
        if let Some(staged) = self.hd_type9_pending {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-9 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply updates only this offline preset value. Discard clears the staged choice.");
                    ui.horizontal(|ui| {
                        let response = ui.button("Apply staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("apply-type9", response.rect);
                        apply = response.clicked();
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", response.rect);
                        cancel = response.clicked();
                        let response = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", response.rect);
                        discard = response.clicked();
                    });
                });
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type9_choice(staged.file_index, staged.slot, staged.choice);
                if self.hd_type9_pending.is_none()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type9_pending = None;
                self.pod_message = "Discarded the staged HD type-9 choice.".into();
                if !self.hd_library.any_dirty() && !self.pod_library.any_dirty() {
                    self.continue_pending(ctx);
                }
            }
            return;
        }
        let Some(index) = self.hd_library.first_dirty() else {
            self.continue_pending(ctx);
            return;
        };
        let kind = self
            .hd_library
            .file(index)
            .and_then(PodPresetFile::hd_container_kind)
            .unwrap_or("HD editor file");
        let remaining = self.hd_library.dirty_count();
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved HD Edit file")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "The selected {kind} has an offline edit that is not saved. {remaining} HD file(s) are unsaved."
                ));
                ui.label("Save writes a new copy only; no device operation is available.");
                ui.horizontal(|ui| {
                    let response = ui.button("Save copy and continue…");
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this edit");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            if self.save_next_dirty_pod()
                && !self.pod_library.any_dirty()
                && !self.hd_library.any_dirty()
            {
                self.continue_pending(ctx);
            }
        } else if discard
            && self.discard_next_dirty_pod()
            && !self.pod_library.any_dirty()
            && !self.hd_library.any_dirty()
        {
            self.continue_pending(ctx);
        }
    }

    fn save_next_dirty_pod(&mut self) -> bool {
        if let Some(index) = self.pod_library.first_dirty() {
            self.write_pod_copy(index)
        } else if let Some(index) = self.hd_library.first_dirty() {
            self.write_hd_copy(index)
        } else {
            false
        }
    }

    fn discard_next_dirty_pod(&mut self) -> bool {
        if let Some(index) = self.pod_library.first_dirty() {
            return match self.pod_library.revert_unsaved(index) {
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
            };
        }
        let Some(index) = self.hd_library.first_dirty() else {
            return false;
        };
        match self.hd_library.revert_unsaved(index) {
            Ok(()) => {
                if self
                    .hd_type9_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type9_pending = None;
                }
                self.pod_message = "Discarded one unsaved HD Edit file change.".into();
                true
            }
            Err(error) => {
                self.pod_message = error.detail();
                false
            }
        }
    }

    fn refresh_selected_pod_name(&mut self) {
        if let Some(index) = self.pod_selected
            && let Some(tone) = self.pod_library.tone(index)
        {
            self.pod_name = tone.name().to_owned();
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
        let mut open = true;
        egui::Window::new("Offline POD preset library")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 64.0))
            .show(ctx, |ui| {
                if self.pending.is_some() {
                    ui.disable();
                }
                ui.label(pod_preset::OFFLINE_BANNER);
                ui.label(pod_preset::NOT_GUITAR);
                ui.label(pod_preset::NOT_SENT);
                ui.label(pod_preset::PAYLOADS_NOT_DECODED);
                ui.label(&self.pod_message);
                if self.pod_button(ui, "load-hd-file", "Open HD Edit file…") {
                    self.load_hd_editor_file();
                }
                let selected_enable = self.hd_selected.and_then(|index| {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot).ok()?;
                    let PodPresetFile::H5ep(preset) = self.hd_library.file(index)? else {
                        return None;
                    };
                    Some((index, slot, preset.effect_enabled(slot)?))
                });
                if let Some((index, slot, enabled)) = selected_enable {
                    let (button_id, label) = if enabled {
                        ("disable-hd-effect", "Disable effect offline")
                    } else {
                        ("enable-hd-effect", "Enable effect offline")
                    };
                    if self.pod_button(ui, button_id, label) {
                        let result = Session::extension(ProductProfile::PodHd500x).and_then(
                            |session| {
                                session.set_hd_effect_enabled(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                    !enabled,
                                )
                            },
                        );
                        match result {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Effect {} updated in this offline preset. Nothing was sent; save a new copy to keep the change.",
                                    if enabled { "disabled" } else { "enabled" }
                                );
                            }
                            Err(error) => self.pod_message = rejection_text(&error),
                        }
                    }
                }
                if let Some(pending) = self.hd_type9_pending
                    && self.hd_selected == Some(pending.file_index)
                    && self.hd_effect_slot == pending.slot.index()
                {
                    ui.label(format!(
                        "{} for FX slot {} is staged.",
                        pending.choice.label(),
                        pending.slot.index()
                    ));
                    ui.horizontal(|ui| {
                        if self.pod_button(
                            ui,
                            "apply-hd-type9-choice",
                            "Apply offline choice",
                        ) {
                            self.apply_hd_type9_choice(
                                pending.file_index,
                                pending.slot,
                                pending.choice,
                            );
                        }
                        if self.pod_button(
                            ui,
                            "discard-hd-type9-choice",
                            "Discard staged choice",
                        ) {
                            self.hd_type9_pending = None;
                            self.pod_message = "Discarded the staged HD type-9 choice.".into();
                        }
                    });
                }
                if self.hd_selected.is_some_and(|index| {
                    self.hd_library
                        .file(index)
                        .is_some_and(|file| file.hd_container_kind().is_some())
                }) && self.pod_button(ui, "save-hd-copy", "Save HD file copy…")
                {
                    self.write_hd_copy(self.hd_selected.expect("selected HD file"));
                }
                let selected_octave_fuzz = self.hd_selected.and_then(|index| {
                    self.hd_library.file(index).and_then(|file| match file {
                        PodPresetFile::H5ep(preset) => {
                            preset.effect_model_id(self.hd_effect_slot)
                        }
                        _ => None,
                    })
                }) == Some(
                    pod_preset::HdType1Control::OctaveFuzz(
                        pod_preset::OctaveFuzzControl::Drive,
                    )
                    .model_id(),
                );
                if selected_octave_fuzz {
                    ui.label("This explicit action replaces the current model and resets its five source-defined values to Vintage Pre defaults.");
                    if self.pod_button(
                        ui,
                        "replace-octave-fuzz-vintage-pre",
                        "Replace Octave Fuzz with Vintage Pre defaults",
                    ) {
                        let index = self.hd_selected.expect("selected HD file");
                        match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                            Ok(slot) => match Session::initial_rig()
                                .replace_hd_octave_fuzz_with_vintage_pre_defaults(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                )
                            {
                                Ok(()) => {
                                    self.pod_message = "Replaced this offline effect with Vintage Pre defaults. Nothing was sent; save a new copy to keep the change.".into();
                                }
                                Err(error) => self.pod_message = rejection_text(&error),
                            },
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                    ui.label("This explicit action replaces the current model and resets its five source-defined values to Screamer defaults.");
                    if self.pod_button(
                        ui,
                        "replace-octave-fuzz-screamer",
                        "Replace Octave Fuzz with Screamer defaults",
                    ) {
                        let index = self.hd_selected.expect("selected HD file");
                        match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                            Ok(slot) => match Session::initial_rig()
                                .replace_hd_octave_fuzz_with_screamer_defaults(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                )
                            {
                                Ok(()) => {
                                    self.pod_message = "Replaced this offline effect with Screamer defaults. Nothing was sent; save a new copy to keep the change.".into();
                                }
                                Err(error) => self.pod_message = rejection_text(&error),
                            },
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                    ui.label("This explicit action replaces the current model and resets its five source-defined values to Sub Octave Fuzz defaults.");
                    if self.pod_button(
                        ui,
                        "replace-octave-fuzz-sub-octave-fuzz",
                        "Replace Octave Fuzz with Sub Octave Fuzz defaults",
                    ) {
                        let index = self.hd_selected.expect("selected HD file");
                        match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                            Ok(slot) => match Session::initial_rig()
                                .replace_hd_octave_fuzz_with_sub_octave_fuzz_defaults(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                )
                            {
                                Ok(()) => {
                                    self.pod_message = "Replaced this offline effect with Sub Octave Fuzz defaults. Nothing was sent; save a new copy to keep the change.".into();
                                }
                                Err(error) => self.pod_message = rejection_text(&error),
                            },
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                    ui.label("Additional source-derived distortion/fuzz defaults");
                    egui::ComboBox::from_id_salt("hd-octave-fuzz-default-target")
                        .selected_text(self.hd_default_target.label())
                        .show_ui(ui, |ui| {
                            for target in pod_preset::HdDistortionDefault::ALL {
                                ui.selectable_value(
                                    &mut self.hd_default_target,
                                    target,
                                    target.label(),
                                );
                            }
                        });
                    if self.pod_button(
                        ui,
                        "replace-octave-fuzz-admitted-default",
                        "Apply selected source defaults",
                    ) {
                        let index = self.hd_selected.expect("selected HD file");
                        match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                            Ok(slot) => match Session::initial_rig()
                                .replace_hd_octave_fuzz_with_admitted_defaults(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                    self.hd_default_target,
                                )
                            {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                        self.hd_default_target.label()
                                    );
                                }
                                Err(error) => self.pod_message = rejection_text(&error),
                            },
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                    ui.separator();
                }
                ui.separator();
                let rows: Vec<_> = (0..self.pod_library.len())
                    .filter_map(|index| {
                        let tone = self.pod_library.tone(index)?;
                        Some((
                            index,
                            format!(
                                "{index} {} {} amp {} cab {}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                            ),
                            tone.name().to_owned(),
                        ))
                    })
                    .collect();
                for (index, label, name) in rows {
                    if ui
                        .selectable_label(self.pod_selected == Some(index), label)
                        .clicked()
                    {
                        self.pod_selected = Some(index);
                        self.pod_name = name;
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("PINF name");
                    ui.text_edit_singleline(&mut self.pod_name);
                });
                if self.pod_button(ui, "load-pod", "Load tone file…") {
                    self.load_pod_tone();
                }
                if self.pod_button(ui, "apply-pod-name", "Apply name") {
                    self.apply_pod_name();
                }
                if self.pod_button(ui, "save-pod", "Save copy…") {
                    self.save_pod_tone();
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
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        pod_preset::OFFLINE_BANNER,
                        pod_preset::NOT_GUITAR,
                        pod_preset::NOT_SENT,
                        pod_preset::PAYLOADS_NOT_DECODED,
                        pod_preset::HD_EDITOR_OFFLINE_NOTICE,
                        self.pod_message,
                    );
                    if let Some(selected) = selected {
                        self.pod_visible.push_str(&selected);
                    }
                }
                ui.separator();
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("pod-hd-editor-panel")
                    .show(ui, |ui| self.hd_editor_panel(ui));
                #[cfg(test)]
                {
                    self.pod_scroll_area =
                        Some((scroll.id, scroll.inner_rect, scroll.content_size.y));
                }
                #[cfg(not(test))]
                let _ = scroll;
            });
        // Closing this window keeps the in-memory library, including unsaved names.
        self.pod_open = open;
    }

    fn hd_editor_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("POD HD Edit files · offline");
        ui.label(pod_preset::HD_EDITOR_OFFLINE_NOTICE);
        ui.label("Fixed H5EP/H5ES/H5EB framing and selected source-traced type-1 and numeric controls are supported across nine effect models. Auxiliary widgets and all other rows remain opaque; file association does not prove physical target compatibility.");
        ui.label("No device, audio, firmware or driver operation is available here. Copy saves never replace an existing file.");
        let rows: Vec<_> = (0..self.hd_library.len())
            .filter_map(|index| {
                let file = self.hd_library.file(index)?;
                Some((
                    index,
                    format!(
                        "{index} · {} · {} bytes{}",
                        file.hd_container_kind()?,
                        file.as_bytes().len(),
                        if self.hd_library.is_dirty(index) {
                            " · unsaved"
                        } else {
                            ""
                        },
                    ),
                ))
            })
            .collect();
        for (index, label) in rows {
            if ui
                .selectable_label(self.hd_selected == Some(index), label)
                .clicked()
            {
                self.select_hd_file(index);
            }
        }

        let Some(index) = self.hd_selected else {
            return;
        };
        let Some(file) = self.hd_library.file(index).cloned() else {
            self.hd_selected = None;
            return;
        };
        ui.label("The editor-group/file-format association is static evidence; internal context words are not treated as a product ID.");
        match &file {
            PodPresetFile::H5ep(preset) => {
                ui.label("Single preset. Source header and unmapped body bytes are preserved.");
                let slot_changed = ui
                    .add(egui::Slider::new(&mut self.hd_effect_slot, 0..=7).text("Effect slot"))
                    .changed();
                if slot_changed {
                    if self.hd_type9_pending.take().is_some() {
                        self.pod_message =
                            "Cleared the staged type-9 choice after changing FX slot.".into();
                    }
                    let selected_model_id = preset.effect_model_id(self.hd_effect_slot);
                    if let Some(model_id) = selected_model_id
                        && let Some(control) = pod_preset::HdType1Control::ALL
                            .into_iter()
                            .find(|control| control.model_id() == model_id)
                    {
                        self.hd_control = control;
                    }
                    self.hd_control_percent = preset
                        .hd_type1_control_percent(self.hd_effect_slot, self.hd_control)
                        .unwrap_or(0);
                    if let Some(model_id) = selected_model_id
                        && let Some(control) = pod_preset::HD_TYPE1_MAPPED_CONTROLS
                            .into_iter()
                            .find(|control| {
                                control.model_id() == model_id
                                    && preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            *control,
                                        )
                                        .is_some()
                            })
                    {
                        self.hd_mapped_control = control;
                        self.hd_control_percent = preset
                            .hd_type1_mapped_control_percent(self.hd_effect_slot, control)
                            .unwrap_or(self.hd_control_percent);
                    }
                    if let Some(control) =
                        pod_preset::HD_NUMERIC_CONTROLS.into_iter().find(|control| {
                            control.model_id()
                                == preset
                                    .effect_model_id(self.hd_effect_slot)
                                    .unwrap_or_default()
                                && preset
                                    .hd_numeric_control_value(self.hd_effect_slot, *control)
                                    .is_some()
                        })
                    {
                        self.hd_numeric_control = control;
                        self.hd_numeric_value = preset
                            .hd_numeric_control_value(self.hd_effect_slot, control)
                            .unwrap_or(control.min());
                    }
                }
                let model_id = preset
                    .effect_model_id(self.hd_effect_slot)
                    .unwrap_or_default();
                if self.hd_type9_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type9_pending = None;
                    self.pod_message =
                        "Cleared a stale staged type-9 choice after the selected model changed."
                            .into();
                }
                if model_id == pod_preset::HD_TYPE9_CHOICE_MODEL_ID {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot)
                        .expect("the visible FX slot slider is bounded to 0..=7");
                    let value = preset.hd_type9_choice_value(slot);
                    let staged = self.hd_type9_pending.filter(|pending| {
                        pending.file_index == index
                            && pending.slot == slot
                            && pending.model_id == model_id
                    });
                    let current_choice = match value {
                        pod_preset::HdType9ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    };
                    let mut selected = staged.map(|pending| pending.choice).or(current_choice);
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType9ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType9ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType9ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType9ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Recovered type-9 choice");
                    let _response = egui::ComboBox::from_id_salt((
                        "hd-type9-choice",
                        index,
                        self.hd_effect_slot,
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType9ChoiceValue::Choice(_)
                                    | pod_preset::HdType9ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType9Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.pod_buttons.insert(
                                        match choice {
                                            pod_preset::HdType9Choice::Choice0 => {
                                                "hd-type9-choice-option-0"
                                            }
                                            pod_preset::HdType9Choice::Choice1 => {
                                                "hd-type9-choice-option-1"
                                            }
                                            pod_preset::HdType9Choice::Choice2 => {
                                                "hd-type9-choice-option-2"
                                            }
                                        },
                                        response.rect,
                                    );
                                    if response.changed() {
                                        self.stage_hd_type9_choice(index, slot, choice);
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.pod_buttons
                        .insert("hd-type9-choice-combo", _response.response.rect);
                    ui.label("Choice labels are neutral. Imported unknown values are preserved until you choose an explicit value.");
                    if matches!(value, pod_preset::HdType9ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                }
                let has_mapped_type1 = pod_preset::HD_TYPE1_MAPPED_CONTROLS.iter().any(|control| {
                    control.model_id() == model_id
                        && preset
                            .hd_type1_mapped_control_percent(self.hd_effect_slot, *control)
                            .is_some()
                });
                if has_mapped_type1 {
                    egui::ComboBox::from_label("Recovered effect control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_mapped_control.model_label(),
                            self.hd_mapped_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HD_TYPE1_MAPPED_CONTROLS {
                                if control.model_id() != model_id
                                    || preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            control,
                                        )
                                        .is_none()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_mapped_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_control_percent = preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            control,
                                        )
                                        .unwrap_or(0);
                                }
                            }
                        });
                    ui.add(
                        egui::Slider::new(&mut self.hd_control_percent, 0..=100).text(format!(
                            "{} {}",
                            self.hd_mapped_control.model_label(),
                            self.hd_mapped_control.label()
                        )),
                    );
                    if self.pod_button(ui, "apply-hd-mapped-type1-control", "Apply offline control")
                    {
                        self.apply_hd_mapped_control(index);
                    }
                } else {
                    egui::ComboBox::from_label("Recovered control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_control.model_label(),
                            self.hd_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HdType1Control::ALL {
                                if control.model_id()
                                    != preset
                                        .effect_model_id(self.hd_effect_slot)
                                        .unwrap_or_default()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_control_percent = preset
                                        .hd_type1_control_percent(self.hd_effect_slot, control)
                                        .unwrap_or(0);
                                }
                            }
                        });
                    if preset
                        .hd_type1_control_percent(self.hd_effect_slot, self.hd_control)
                        .is_some()
                    {
                        ui.add(
                            egui::Slider::new(&mut self.hd_control_percent, 0..=100).text(format!(
                                "{} {}",
                                self.hd_control.model_label(),
                                self.hd_control.label()
                            )),
                        );
                        if self.pod_button(ui, "apply-hd-drive", "Apply offline control") {
                            self.apply_hd_control(index);
                        }
                    } else {
                        ui.label(
                            "No matching type-1 control is available; other rows remain unchanged.",
                        );
                    }
                }
                if pod_preset::HD_NUMERIC_CONTROLS.iter().any(|control| {
                    control.model_id()
                        == preset
                            .effect_model_id(self.hd_effect_slot)
                            .unwrap_or_default()
                        && preset
                            .hd_numeric_control_value(self.hd_effect_slot, *control)
                            .is_some()
                }) {
                    egui::ComboBox::from_label("Recovered numeric control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_numeric_control.model_label(),
                            self.hd_numeric_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HD_NUMERIC_CONTROLS {
                                if control.model_id()
                                    != preset
                                        .effect_model_id(self.hd_effect_slot)
                                        .unwrap_or_default()
                                    || preset
                                        .hd_numeric_control_value(self.hd_effect_slot, control)
                                        .is_none()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_numeric_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_numeric_value = preset
                                        .hd_numeric_control_value(self.hd_effect_slot, control)
                                        .unwrap_or(control.min());
                                }
                            }
                        });
                    ui.label("Editor integer control values; source labels and bounds do not declare physical units.");
                    ui.add(
                        egui::Slider::new(
                            &mut self.hd_numeric_value,
                            self.hd_numeric_control.min()..=self.hd_numeric_control.max(),
                        )
                        .text(format!(
                            "{} {}",
                            self.hd_numeric_control.model_label(),
                            self.hd_numeric_control.label()
                        )),
                    );
                    if self.pod_button(
                        ui,
                        "apply-hd-numeric-control",
                        "Apply offline numeric control",
                    ) {
                        self.apply_hd_numeric_control(index);
                    }
                }
            }
            PodPresetFile::H5es(setlist) => {
                ui.label(format!("One setlist with {0} fixed preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.pod_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        0,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.pod_message = rejection_text(&error),
                    }
                }
                let _ = setlist;
            }
            PodPresetFile::H5eb(bundle) => {
                ui.label(format!("Bundle with {0} fixed setlists × {1} preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5EB_SETLIST_COUNT, pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_setlist_index, 0..=7).text("Setlist index"));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.pod_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        self.hd_setlist_index,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.pod_message = rejection_text(&error),
                    }
                }
                let _ = bundle;
            }
            PodPresetFile::PodxtLive(_) => return,
        }
        ui.label(&self.pod_message);
    }

    fn select_hd_file(&mut self, index: usize) {
        if self.hd_type9_pending.take().is_some() {
            self.pod_message =
                "Cleared a staged type-9 choice after changing HD file selection.".into();
        }
        self.hd_selected = Some(index);
        self.hd_effect_slot = 0;
        self.hd_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HdType1Control::ALL
                        .into_iter()
                        .find(|control| control.model_id() == model)
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HdType1Control::Screamer(
                pod_preset::ScreamerControl::Drive,
            ));
        self.hd_control_percent = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => preset.hd_type1_control_percent(0, self.hd_control),
                _ => None,
            })
            .unwrap_or(0);
        self.hd_mapped_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HD_TYPE1_MAPPED_CONTROLS
                        .into_iter()
                        .find(|control| {
                            control.model_id() == model
                                && preset
                                    .hd_type1_mapped_control_percent(0, *control)
                                    .is_some()
                        })
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HD_TYPE1_MAPPED_CONTROLS[0]);
        self.hd_control_percent = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    preset.hd_type1_mapped_control_percent(0, self.hd_mapped_control)
                }
                _ => None,
            })
            .unwrap_or(self.hd_control_percent);
        self.hd_numeric_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HD_NUMERIC_CONTROLS.into_iter().find(|control| {
                        control.model_id() == model
                            && preset.hd_numeric_control_value(0, *control).is_some()
                    })
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HD_NUMERIC_CONTROLS[0]);
        self.hd_numeric_value = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    preset.hd_numeric_control_value(0, self.hd_numeric_control)
                }
                _ => None,
            })
            .unwrap_or(self.hd_numeric_control.min());
    }

    fn apply_hd_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_type1_control_percent(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_control,
            self.hd_control_percent,
        );
        match result {
            Ok(()) => {
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_mapped_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_type1_mapped_control_percent(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_mapped_control,
            self.hd_control_percent,
        );
        match result {
            Ok(()) => {
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_numeric_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_numeric_control_value(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_numeric_control,
            self.hd_numeric_value,
        );
        match result {
            Ok(()) => {
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_type9_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        choice: pod_preset::HdType9Choice,
    ) {
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type9_choice(&mut self.hd_library, file_index, slot, choice)
        });
        match result {
            Ok(()) => {
                if self.hd_type9_pending.is_some_and(|pending| {
                    pending.file_index == file_index
                        && pending.slot == slot
                        && pending.choice == choice
                }) {
                    self.hd_type9_pending = None;
                }
                self.pod_message = "Applied the type-9 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn stage_hd_type9_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        choice: pod_preset::HdType9Choice,
    ) {
        self.hd_type9_pending = Some(PendingHdType9Choice {
            file_index,
            slot,
            model_id: pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
            choice,
        });
    }

    fn load_hd_editor_file(&mut self) {
        let Some(path) = self.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        let facts = pod_preset::path_facts(&path);
        let bytes = match facts {
            Ok(facts) if facts.len <= pod_preset::TONE_BYTES_CEILING => {
                pod_preset::read_regular_tone(Ok(facts), || pod_preset::open_regular_file(&path))
            }
            Ok(facts) => pod_preset::read_regular_hd_editor_file(Ok(facts), || {
                pod_preset::open_regular_file(&path)
            }),
            Err(error) => Err(pod_preset::ToneError::Format {
                detail: error.to_string(),
            }),
        };
        match bytes.and_then(|bytes| {
            let parsed = PodPresetFile::parse(bytes.clone())?;
            Ok((bytes, parsed))
        }) {
            Ok((bytes, PodPresetFile::PodxtLive(_)))
                if bytes.len() as u64 <= pod_preset::TONE_BYTES_CEILING =>
            {
                match Session::initial_rig().open_pod_preset(RoleKind::Pod, &bytes) {
                    Ok(summary) => match self.pod_library.import_bytes(bytes) {
                        Ok(index) => {
                            self.pod_selected = Some(index);
                            self.pod_name = summary.name;
                            self.pod_message = format!(
                                "Loaded {} for PODxt Live {:08x}. {}",
                                summary.container,
                                summary.device_id,
                                pod_preset::PAYLOADS_NOT_DECODED
                            );
                        }
                        Err(error) => self.pod_message = error.detail(),
                    },
                    Err(error) => self.pod_message = rejection_text(&error),
                }
            }
            Ok((bytes, _)) => {
                match Session::initial_rig().import_hd_editor_file(&mut self.hd_library, bytes) {
                    Ok(summary) => {
                        self.select_hd_file(summary.index);
                        self.pod_message = "Loaded an offline HD Edit file. Internal product target and physical compatibility remain unverified.".into();
                    }
                    Err(error) => self.pod_message = rejection_text(&error),
                }
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn write_hd_copy(&mut self, index: usize) -> bool {
        let Some(file) = self.hd_library.file(index) else {
            self.pod_message = "Select an HD Edit file before saving a copy.".into();
            return false;
        };
        let suggested = match file.hd_container_kind() {
            Some("H5ES setlist") => "hd-edit-setlist.5xs",
            Some("H5EB bundle") => "hd-edit-bundle.5xb",
            _ => "hd-edit-preset.5xe",
        };
        let Some(path) = self.dialogs.save_pod_tone(suggested) else {
            self.pod_message = "Save cancelled. No file was written.".into();
            return false;
        };
        match Session::initial_rig().save_hd_editor_copy(&self.hd_library, &path, index) {
            Ok(()) => match self.hd_library.note_saved(index) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Saved a new offline copy to {}. No device operation occurred.",
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

    fn pod_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.pod_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn load_pod_tone(&mut self) {
        let Some(path) = self.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        match pod_preset::read_regular_tone(pod_preset::path_facts(&path), || {
            pod_preset::open_regular_file(&path)
        }) {
            Ok(bytes) => match Session::initial_rig().open_pod_preset(RoleKind::Pod, &bytes) {
                Ok(summary) => match self.pod_library.import_bytes(bytes) {
                    Ok(index) => {
                        self.pod_selected = Some(index);
                        self.pod_name = summary.name;
                        self.pod_message = format!(
                            "Loaded {} for PODxt Live {:08x}. {}",
                            summary.container,
                            summary.device_id,
                            pod_preset::PAYLOADS_NOT_DECODED
                        );
                    }
                    Err(error) => self.pod_message = error.detail(),
                },
                Err(error) => self.pod_message = rejection_text(&error),
            },
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
            Ok(bytes) => match self.pod_library.replace(index, bytes) {
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

    fn save_pod_tone(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return;
        };
        self.write_pod_copy(index);
    }

    fn write_pod_copy(&mut self, index: usize) -> bool {
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
        let pos = app
            .pod_buttons
            .get(id)
            .unwrap_or_else(|| {
                panic!(
                    "expected rendered POD button {id:?}; registered: {:?}",
                    app.pod_buttons.keys().collect::<Vec<_>>()
                )
            })
            .center();
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

    fn hd_editor_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn click_hd_editor(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        hd_editor_frame(context, app, vec![]);
        if !matches!(
            id,
            "apply-hd-type9-choice"
                | "discard-hd-type9-choice"
                | "save-hd-copy"
                | "load-hd-file"
                | "hd-type9-choice-option-0"
                | "hd-type9-choice-option-1"
                | "hd-type9-choice-option-2"
        ) {
            scroll_hd_editor_control_into_view(context, app, id);
            hd_editor_frame(context, app, vec![]);
        }
        let pos = app
            .pod_buttons
            .get(id)
            .unwrap_or_else(|| {
                panic!(
                    "expected rendered POD button {id:?}; registered: {:?}",
                    app.pod_buttons.keys().collect::<Vec<_>>()
                )
            })
            .center();
        hd_editor_frame(
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
        hd_editor_frame(
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

    fn scroll_hd_editor_control_into_view(
        context: &egui::Context,
        app: &Workbench,
        id: &'static str,
    ) {
        let rect = app.pod_buttons[id];
        let (scroll_id, viewport, content_height) =
            app.pod_scroll_area.expect("rendered HD editor scroll area");
        let mut state = egui::scroll_area::State::load(context, scroll_id).unwrap_or_default();
        let max_offset = (content_height - viewport.height()).max(0.0);
        state.offset.y =
            (state.offset.y + rect.center().y - viewport.center().y).clamp(0.0, max_offset);
        state.store(context, scroll_id);
    }

    fn click_hd_type9_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        click_hd_editor(context, app, "hd-type9-choice-combo");
        let option_id = match choice {
            0 => "hd-type9-choice-option-0",
            1 => "hd-type9-choice-option-1",
            2 => "hd-type9-choice-option-2",
            _ => panic!("invalid type-9 choice index"),
        };
        click_hd_editor(context, app, option_id);
    }

    fn assert_hd_bytes_eq(actual: &[u8], expected: &[u8], context: &str) {
        let differences = actual
            .iter()
            .zip(expected)
            .enumerate()
            .filter_map(|(offset, (actual, expected))| {
                (actual != expected).then_some((offset, *actual, *expected))
            })
            .take(8)
            .collect::<Vec<_>>();
        assert!(
            actual.len() == expected.len() && differences.is_empty(),
            "{context}: actual length {}, expected length {}, first differences (offset, actual, expected): {differences:?}",
            actual.len(),
            expected.len()
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

    fn sample_hd_editor_preset() -> Vec<u8> {
        let mut bytes = vec![0; pod_preset::H5EP_MIN_BYTES];
        bytes[..4].copy_from_slice(b"H5EP");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        let body = &mut bytes[pod_preset::H5EP_HEADER_BYTES..];
        for offset in [0x228, 0x328] {
            body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
        }
        for index in 0..8 {
            let block = 0x428 + index * 0x100;
            body[block..block + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
        }
        let block = 0x428;
        body[block..block + 4].copy_from_slice(&0x0205_000du32.to_be_bytes());
        let row = block + 0x10;
        body[row..row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
        body[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        let bass_row = row + 0x14;
        body[bass_row..bass_row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
        body[bass_row + 4..bass_row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
        let mid_row = bass_row + 0x14;
        body[mid_row..mid_row + 4].copy_from_slice(&0x3f10_0003u32.to_be_bytes());
        body[mid_row + 4..mid_row + 8].copy_from_slice(&0.35f32.to_bits().to_be_bytes());
        bytes
    }

    fn sample_hd_type9_preset(slot: usize, row: usize, bits: u32) -> Vec<u8> {
        let mut bytes = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
        bytes[block..block + 4]
            .copy_from_slice(&pod_preset::HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
        bytes[block + 8] = 0;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        let row_at = block + 0x10 + row * 0x14;
        bytes[row_at..row_at + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].fill(0x31);
        bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
        bytes.extend_from_slice(b"type9-tail");
        bytes
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
        assert!(app.pod_visible.contains("not decoded"));
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
        assert!(app.pod_visible.contains("not decoded"));
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
    fn hd_editor_library_workflow_edits_and_saves_a_new_copy_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.5xe");
        let output = directory.path().join("copy.5xe");
        let blocked_output = directory.path().join("blocked-output");
        std::fs::create_dir(&blocked_output).unwrap();
        let sentinel = blocked_output.join("keep.bin");
        std::fs::write(&sentinel, b"keep").unwrap();
        let original = sample_hd_editor_preset();
        std::fs::write(&input, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(input.clone())].into(),
            pod_save: [Some(blocked_output.clone()), Some(output.clone())].into(),
            ..Default::default()
        });

        app.load_hd_editor_file();
        assert_eq!(app.hd_library.len(), 1);
        assert_eq!(app.pod_library.len(), 0);
        assert_eq!(app.hd_selected, Some(0));
        assert_eq!(app.hd_control_percent, 50);
        app.hd_control_percent = 75;
        app.apply_hd_control(0);
        app.hd_control = pod_preset::HdType1Control::TubeDrive(pod_preset::TubeDriveControl::Mid);
        assert_eq!(
            app.hd_library.file(0).and_then(|file| match file {
                PodPresetFile::H5ep(preset) =>
                    preset.tube_drive_control_percent(0, pod_preset::TubeDriveControl::Mid),
                _ => None,
            }),
            Some(35)
        );
        app.hd_control_percent = 40;
        app.apply_hd_control(0);
        assert!(app.hd_library.is_dirty(0));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source);
        assert_eq!(std::fs::read(&input).unwrap(), original);

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(!app.permit_close);
        assert!(app.hd_library.is_dirty(0));
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let written = std::fs::read(&output).unwrap();
        assert_eq!(written.len(), original.len());
        assert_eq!(
            &written[0x28 + 0x428 + 0x10 + 4..0x28 + 0x428 + 0x10 + 8],
            &0.75f32.to_bits().to_be_bytes()
        );
        assert_eq!(
            &written[0x28 + 0x428 + 0x10 + 2 * 0x14 + 4..0x28 + 0x428 + 0x10 + 2 * 0x14 + 8],
            &0.4f32.to_bits().to_be_bytes()
        );
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    fn exercise_hd_editor_explicit_replacement_lifecycle(
        target_model: u32,
        default_keys: [u32; 5],
        button_id: &'static str,
        message_label: &str,
        output_name: &str,
        batch_target: Option<pod_preset::HdDistortionDefault>,
    ) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block..block + 4].copy_from_slice(&0x0205_000cu32.to_be_bytes());
        original[block + 4..block + 8].copy_from_slice(&0x1234_5678u32.to_be_bytes());
        original[block + 8..block + 0x0c].copy_from_slice(&[1, 0x5a, 0xa5, 1]);
        original[block + 0x0f] = 8;
        let keys = [
            0x3f10_0005u32,
            0x3f10_0001,
            0x3f10_0003,
            0x3f10_0002,
            0x3f10_0004,
        ];
        for (row, key) in keys.iter().copied().enumerate().chain((5..12).map(|row| {
            let key = if row == 6 {
                0x3f10_0005
            } else {
                0x6abc_0000 + row as u32
            };
            (row, key)
        })) {
            let row_at = block + 0x10 + row * 0x14;
            original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            original[row_at + 4..row_at + 8]
                .copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
            original[row_at + 8..row_at + 12]
                .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
            original[row_at + 0x10] = (row % 3) as u8;
            original[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
                0xc1 + row as u8,
                0xd1 + row as u8,
                0xe1 + row as u8,
            ]);
        }
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        if let Some(target) = batch_target {
            app.hd_default_target = target;
        }
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);

        let mut expected = original.clone();
        expected[block..block + 4].copy_from_slice(&target_model.to_be_bytes());
        for (row, key) in default_keys.into_iter().enumerate() {
            let row_at = block + 0x10 + row * 0x14;
            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        expected[block + 0x0f] = 5;
        for row in 0..12 {
            expected[block + 0x10 + row * 0x14 + 0x10] = 0;
        }

        click_pod(&context, &mut app, button_id);
        assert!(
            app.pod_message.contains(message_label),
            "{}",
            app.pod_message
        );
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);
        app.hd_library.revert_unsaved(index).unwrap();
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);
        assert!(!app.hd_library.is_dirty(index));

        click_pod(&context, &mut app, button_id);
        assert!(app.hd_library.is_dirty(index));
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("blocked-directory");
        std::fs::create_dir(&blocked).unwrap();
        let collision = directory.path().join("existing.5xe");
        let output = directory.path().join(output_name);
        std::fs::write(&collision, b"keep destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                None,
                Some(blocked.clone()),
                Some(collision.clone()),
                Some(output.clone()),
            ]
            .into(),
            ..Default::default()
        });
        assert!(!app.write_hd_copy(index)); // Save dialog cancellation.
        assert!(app.hd_library.is_dirty(index));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));

        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // A directory cannot be the destination.
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        click_prompt(&context, &mut app, "save"); // Existing destination is preserved.
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let written = std::fs::read(&output).unwrap();
        assert_eq!(written, expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(written).unwrap();
        assert_eq!(reopened.effect_model_id(0), Some(target_model));
        assert!(reopened.trailing_bytes().is_empty());
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn hd_effect_enable_ui_edits_one_flag_and_guards_copy_save_and_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block + 0x0b] = 1;
        original[block + 8] = 0;
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("enable-hd-effect"));

        click_hd_editor(&context, &mut app, "enable-hd-effect");
        let mut expected = original.clone();
        expected[block + 8] = 1;
        let actual = app.hd_library.file(index).unwrap().as_bytes();
        let differences = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .filter_map(|(offset, (actual, expected))| (actual != expected).then_some(offset))
            .collect::<Vec<_>>();
        assert!(
            differences.is_empty(),
            "unexpected byte offsets: {differences:?}; actual={}, expected={}, button={:?}; message={}",
            actual[block + 8],
            expected[block + 8],
            app.pod_buttons.get("enable-hd-effect"),
            app.pod_message
        );
        assert!(app.hd_library.is_dirty(index));
        assert!(app.pod_message.contains("Nothing was sent"));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));

        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("blocked-directory");
        std::fs::create_dir(&blocked).unwrap();
        let collision = directory.path().join("existing.5xe");
        let output = directory.path().join("enabled-copy.5xe");
        std::fs::write(&collision, b"keep destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                Some(blocked.clone()),
                Some(collision.clone()),
                Some(output.clone()),
            ]
            .into(),
            ..Default::default()
        });

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let saved = std::fs::read(&output).unwrap();
        assert_eq!(saved, expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(saved).unwrap();
        assert_eq!(
            reopened.effect_enabled(pod_preset::HdFxSlot::new(0).unwrap()),
            Some(true)
        );
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn hd_editor_explicit_default_replacements_stages_reverts_saves_and_guards_quit() {
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x020c_000e,
            [
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
                0x3f10_0001,
            ],
            "replace-octave-fuzz-vintage-pre",
            "Vintage Pre defaults",
            "vintage-pre-copy.5xe",
            None,
        );
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x0205_000e,
            [
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "replace-octave-fuzz-screamer",
            "Screamer defaults",
            "screamer-copy.5xe",
            None,
        );
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x0205_0014,
            [
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "replace-octave-fuzz-sub-octave-fuzz",
            "Sub Octave Fuzz defaults",
            "sub-octave-fuzz-copy.5xe",
            None,
        );

        const BATCH: [(pod_preset::HdDistortionDefault, u32, &str, &str); 12] = [
            (
                pod_preset::HdDistortionDefault::JetFuzz,
                0x0205_000a,
                "Jet Fuzz defaults",
                "jet-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::BuzzSaw,
                0x0205_0015,
                "Buzz Saw defaults",
                "buzz-saw-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::ClassicDistortion,
                0x0205_000b,
                "Classic Distortion defaults",
                "classic-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::ColorDrive,
                0x0205_0018,
                "Color Drive defaults",
                "color-drive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::FacialFuzz,
                0x0205_0011,
                "Facial Fuzz defaults",
                "facial-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::FuzzPi,
                0x0205_000f,
                "Fuzz Pi defaults",
                "fuzz-pi-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::HeavyDistortion,
                0x0205_0016,
                "Heavy Distortion defaults",
                "heavy-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::JumboFuzz,
                0x0205_0017,
                "Jumbo Fuzz defaults",
                "jumbo-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Line6Distortion,
                0x0205_0012,
                "Line 6 Distortion defaults",
                "line6-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Line6Drive,
                0x0205_0013,
                "Line 6 Drive defaults",
                "line6-drive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Overdrive,
                0x0205_0010,
                "Overdrive defaults",
                "overdrive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::TubeDrive,
                0x0205_000d,
                "Tube Drive defaults",
                "tube-drive-copy.5xe",
            ),
        ];
        for (target, model_id, label, output_name) in BATCH {
            exercise_hd_editor_explicit_replacement_lifecycle(
                model_id,
                [
                    0x3f10_0001,
                    0x3f10_0002,
                    0x3f10_0003,
                    0x3f10_0004,
                    0x3f10_0005,
                ],
                "replace-octave-fuzz-admitted-default",
                label,
                output_name,
                Some(target),
            );
        }
    }

    #[test]
    fn hd_editor_numeric_control_updates_only_its_traced_value_offline() {
        let (_context, mut app) = app();
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let mut bytes = sample_hd_editor_preset();
        let body = pod_preset::H5EP_HEADER_BYTES;
        let block = body + 0x428;
        bytes[block..block + 4].copy_from_slice(&0x020c_0009u32.to_be_bytes());
        let controls = pod_preset::HD_NUMERIC_CONTROLS
            .into_iter()
            .filter(|control| control.model_id() == 0x020c_0009);
        for (index, control) in controls.enumerate() {
            let row = block + 0x10 + index * 0x14;
            bytes[row..row + 4].copy_from_slice(
                &match index {
                    0 => 0x3f10_0001u32,
                    1 => 0x3f10_0002u32,
                    2 => 0x3f10_0003u32,
                    3 => 0x3f10_0004u32,
                    _ => 0x3f10_0005u32,
                }
                .to_be_bytes(),
            );
            bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
            bytes[row + 0x10] = 0;
            assert_eq!(control.max(), 240);
        }
        let original = bytes.clone();
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        let control = app.hd_numeric_control;
        assert_eq!(control.model_label(), "Graphic EQ");
        assert_eq!(control.label(), "80Hz");
        assert_eq!(app.hd_numeric_value, 120);
        app.hd_numeric_value = 180;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);
        let edited = app.hd_library.file(index).unwrap().as_bytes();
        let value_at = block + 0x10 + 4;
        assert_eq!(&edited[..value_at], &original[..value_at]);
        assert_eq!(
            &edited[value_at..value_at + 4],
            &0.75f32.to_bits().to_be_bytes()
        );
        assert_eq!(&edited[value_at + 4..], &original[value_at + 4..]);
        assert!(app.read_worker.is_none());
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn hd_editor_vintage_pre_type1b_value_patches_through_gui_save_and_quit_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let control = pod_preset::HD_NUMERIC_CONTROLS
            .into_iter()
            .find(|control| control.raw_type() == 0x1b)
            .unwrap();
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = block + 0x10;
        original[row..row + 4].copy_from_slice(&0x3f10_0005u32.to_be_bytes());
        original[row + 4..row + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
        original[row + 0x10] = 0;
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);

        assert_eq!(app.hd_numeric_control, control);
        assert_eq!(app.hd_numeric_control.raw_type(), 0x1b);
        assert_eq!(
            (app.hd_numeric_control.min(), app.hd_numeric_control.max()),
            (0, 15_000)
        );
        assert_eq!(app.hd_numeric_value, 7_500);

        // The existing library revert restores the exact loaded baseline.
        app.hd_numeric_value = 12_000;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        app.hd_library.revert_unsaved(index).unwrap();
        assert!(!app.hd_library.is_dirty(index));
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);

        app.hd_numeric_value = 14_999;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        let mut expected = original.clone();
        expected[row + 4..row + 8].copy_from_slice(&0x3f7f_fba2u32.to_be_bytes());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing.h5p");
        let output = directory.path().join("vintage-pre-copy.h5p");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(output.clone())].into(),
            ..Default::default()
        });
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert!(app.hd_library.is_dirty(index));
        assert!(!app.permit_close);
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        assert!(app.read_worker.is_none());
        assert_eq!(std::fs::read(&output).unwrap(), expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(std::fs::read(output).unwrap())
            .expect("reopen saved type-0x1B copy");
        assert_eq!(reopened.hd_numeric_control_value(0, control), Some(14_999));
        const {
            assert!(LIVE_ACCESS_LOCKED);
        }
    }

    #[test]
    fn hd_editor_mapped_type1_control_updates_its_traced_value_offline() {
        let (_context, mut app) = app();
        let control = pod_preset::HD_TYPE1_MAPPED_CONTROLS[0];
        let mut bytes = sample_hd_editor_preset();
        let body = pod_preset::H5EP_HEADER_BYTES;
        let block = body + 0x428;
        bytes[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = block + 0x10;
        bytes[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
        bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        bytes[row + 0x10] = 0;
        let original = bytes.clone();
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        assert_eq!(app.hd_mapped_control, control);
        assert_eq!(app.hd_control_percent, 50);

        app.hd_control_percent = 75;
        app.apply_hd_mapped_control(index);
        assert_eq!(
            app.hd_library.file(index).and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    preset.hd_type1_mapped_control_percent(0, control)
                }
                _ => None,
            }),
            Some(75)
        );
        let mut expected = original;
        expected[row + 4..row + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);
        assert!(app.hd_library.is_dirty(index));
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn hd_type9_choice_stages_per_entry_slot_then_applies_or_discards_explicitly() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let row_index = 11;
        let original = sample_hd_type9_preset(slot_index, row_index, 0x3f40_0000);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;
        let slot = pod_preset::HdFxSlot::new(slot_index).unwrap();

        assert!(matches!(
            app.hd_library.file(index),
            Some(PodPresetFile::H5ep(preset))
                if preset.hd_type9_choice_value(slot)
                    == pod_preset::HdType9ChoiceValue::UnknownFinite(0x3f40_0000)
        ));
        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "staging must not alter file bytes",
        );
        assert_eq!(
            app.hd_type9_pending,
            Some(PendingHdType9Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
                choice: pod_preset::HdType9Choice::Choice2,
            })
        );
        click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
        let value_at = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 4;
        let mut expected = original.clone();
        expected[value_at..value_at + 4].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "applying the staged GUI choice",
        );
        assert!(app.hd_library.is_dirty(index));
        assert!(app.hd_type9_pending.is_none());
        assert!(app.pod_message.contains("Nothing was sent"));

        let directory = tempfile::tempdir().unwrap();
        let saved_copy = directory.path().join("type9-copy.5xe");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved_copy.clone())].into(),
            pod_open: [Some(saved_copy.clone())].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_eq!(std::fs::read(&saved_copy).unwrap(), expected);
        assert!(!app.hd_library.is_dirty(index));
        click_hd_editor(&context, &mut app, "load-hd-file");
        let reopened_index = app.hd_selected.expect("GUI open selects the saved copy");
        assert_ne!(reopened_index, index);
        assert_hd_bytes_eq(
            app.hd_library.file(reopened_index).unwrap().as_bytes(),
            &expected,
            "GUI save and reopen",
        );
        app.select_hd_file(index);

        click_hd_type9_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discarding a later staged GUI choice",
        );
        assert!(app.hd_type9_pending.is_none());

        let other_index = app
            .hd_library
            .import_bytes(sample_hd_editor_preset())
            .unwrap();
        app.stage_hd_type9_choice(index, slot, pod_preset::HdType9Choice::Choice1);
        app.select_hd_file(other_index);
        assert!(app.hd_type9_pending.is_none());
        app.select_hd_file(index);
        app.hd_effect_slot = slot_index;
        app.stage_hd_type9_choice(index, slot, pod_preset::HdType9Choice::Choice1);
        app.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type9_pending.is_none());
        app.hd_effect_slot = slot_index;

        click_hd_type9_choice(&context, &mut app, 0);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_type9_pending.is_some());
        assert!(!app.permit_close);

        app.request_close(&context);
        click_prompt(&context, &mut app, "apply-type9");
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(!app.permit_close);

        click_hd_type9_choice(&context, &mut app, 1);
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.hd_library.is_dirty(index));
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discarding edited type-9 value restores saved baseline",
        );
        assert!(app.permit_close);
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn hd_type9_gui_does_not_stage_nonfinite_or_unavailable_values() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        for (index, bits, invalid_marker) in [(0, 0x7fc0_0000, false), (1, 0x3f40_0000, true)] {
            let mut bytes = sample_hd_type9_preset(2, 3, bits);
            if invalid_marker {
                let row = pod_preset::H5EP_HEADER_BYTES + 0x428 + 2 * 0x100 + 0x10 + 3 * 0x14;
                bytes[row + 0x10] = 1;
            }
            let file_index = app.hd_library.import_bytes(bytes.clone()).unwrap();
            assert_eq!(file_index, index);
            app.select_hd_file(file_index);
            app.hd_effect_slot = 2;
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_type9_choice(&context, &mut app, 1);
            assert!(app.hd_type9_pending.is_none());
            assert_hd_bytes_eq(
                app.hd_library.file(file_index).unwrap().as_bytes(),
                &bytes,
                "refused non-finite or unavailable GUI value",
            );
        }

        let mut malformed = sample_hd_type9_preset(2, 3, 0x3f40_0000);
        malformed[7] = 2;
        assert!(app.hd_library.import_bytes(malformed).is_err());
    }

    #[test]
    fn hd_type9_idle_redraw_preserves_the_users_library_scroll_position() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let bytes = sample_hd_type9_preset(0, 3, 0x3f40_0000);
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        hd_editor_frame(&context, &mut app, vec![]);

        let (scroll_id, viewport, content_height) =
            app.pod_scroll_area.expect("rendered HD editor scroll area");
        let max_offset = (content_height - viewport.height()).max(0.0);
        assert!(max_offset > 0.0, "fixture must require vertical scrolling");
        let mut state = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        state.offset.y = max_offset;
        state.store(&context, scroll_id);

        hd_editor_frame(&context, &mut app, vec![]);
        let (scroll_id, viewport, _) = app.pod_scroll_area.unwrap();
        let after_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        assert!((after_idle.offset.y - max_offset).abs() < 1.0);
        assert!(
            !viewport.intersects(app.pod_buttons["hd-type9-choice-combo"]),
            "the scrolled-away type-9 control must stay offscreen after redraw"
        );

        hd_editor_frame(&context, &mut app, vec![]);
        let after_second_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        assert!((after_second_idle.offset.y - after_idle.offset.y).abs() < 1.0);
    }

    fn click_pod_window_close(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let rect = context
            .memory(|memory| memory.area_rect(egui::Id::new("Offline POD preset library")))
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

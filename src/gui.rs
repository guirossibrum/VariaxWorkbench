//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
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
}

struct Workbench {
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
    #[cfg(test)]
    bank_rows: HashMap<(usize, usize), Rect>,
    read_receiver:
        Option<std::sync::mpsc::Receiver<Result<variax_workbench::usb::CurrentPreset, String>>>,
    read_worker: Option<(
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    )>,
}

impl Workbench {
    fn new(cc: &eframe::CreationContext<'_>, preset: PathBuf, artwork: PathBuf) -> Self {
        Self::with_context(&cc.egui_ctx, preset, artwork)
    }

    fn with_context(ctx: &egui::Context, preset: PathBuf, artwork: PathBuf) -> Self {
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
            #[cfg(test)]
            bank_rows: HashMap::new(),
            read_receiver: None,
            read_worker: None,
        };
        app.load(&preset);
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
            && let Some(document) = &mut self.document
        {
            document.undo();
        }
    }

    fn redo(&mut self) {
        if !self.is_reading()
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

    fn open_dialog(&mut self) {
        if self.is_reading() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Workbench model, bank or collection",
                &["vwm", "vwb", "l6c"],
            )
            .pick_file()
        {
            if self.dirty() {
                self.pending = Some(Pending::Open(path));
            } else {
                self.load(&path);
            }
        }
    }

    fn save_dialog(&mut self) {
        if self.is_reading() {
            return;
        }
        let Some(document) = &mut self.document else {
            return;
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
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Workbench document", &[extension])
            .set_file_name(name)
            .save_file()
        {
            match document.save_copy(&path) {
                Ok(()) => {
                    self.error = None;
                    self.status = format!("Saved copy: {}", path.display());
                }
                Err(error) => self.error = Some(error),
            }
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
                ui.add_enabled_ui(!self.is_reading(), |ui| {
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
        if self.is_reading() {
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
                self.error.get_or_insert_with(|| format!("Original artwork not found at {}. Choose the editor image folder with Artwork folder.", self.artwork.display()));
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
        ui.weak("Original Workbench artwork · drag pickups to position them");
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
                if let Err(error) = image::save_buffer(
                    &path,
                    &pixels,
                    capture.size[0] as u32,
                    capture.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.error = Some(format!("Screenshot: {error}"));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
        if save {
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
            if self.dirty() && !self.permit_close {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.pending = Some(Pending::Quit);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
                    if ui.add_enabled(!self.is_reading(), egui::Button::new("Save copy…")).clicked() {
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
                    if ui.add_enabled(!self.is_reading(), egui::Button::new("Open…")).clicked() {
                        self.open_dialog();
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
            let can_edit = editable && !self.is_reading();
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
                    if ui.small_button("Artwork folder…").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder() { self.artwork = path; self.textures.clear(); self.missing_textures.clear(); self.error = None; }
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
                        if ui.button("Save copy and continue…").clicked() {
                            save_continue = true;
                        }
                        if ui.button("Keep editing").clicked() {
                            cancel = true;
                        }
                        if ui.button("Discard changes").clicked() {
                            discard = true;
                        }
                    });
                });
            if cancel {
                self.pending = None;
            }
            if save_continue {
                self.save_dialog();
                discard = !self.dirty();
            }
            if discard {
                match self.pending.take().unwrap() {
                    Pending::Open(path) => self.load(&path),
                    Pending::ReadCurrent => self.launch_read(),
                    Pending::Quit => {
                        self.permit_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
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

const USAGE: &str = "Usage: variax-workbench-gui [--preset FILE.vwm] [--artwork EDITOR_IMAGE_FOLDER] [--tab body|pickups|tuning|controls] [--screenshot OUTPUT.png]";

struct GuiOptions {
    preset: PathBuf,
    artwork: PathBuf,
    screenshot: Option<PathBuf>,
    tab: Tab,
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    root: &Path,
) -> Result<GuiOptions, String> {
    let mut options = GuiOptions {
        preset: root.join("analysis/extracted/windows-app/modules/variax/device/Default Patch.vwm"),
        artwork: root.join("assets/original-ui/modules/variax/editor/images"),
        screenshot: None,
        tab: Tab::Body,
    };
    let mut args = arguments.into_iter();
    while let Some(arg) = args.next() {
        if !matches!(
            arg.as_str(),
            "--preset" | "--artwork" | "--screenshot" | "--tab"
        ) {
            return Err(format!("Unknown argument: {arg}"));
        }
        let value = args
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("{arg} requires a value"))?;
        match arg.as_str() {
            "--preset" => options.preset = PathBuf::from(value),
            "--artwork" => options.artwork = PathBuf::from(value),
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
    Ok(options)
}

fn main() -> eframe::Result {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let arguments = match parse_options(arguments, Path::new(env!("CARGO_MANIFEST_DIR"))) {
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
            root.join("tests/fixtures/editor-preset.vwm"),
            root.join("assets/original-ui/modules/variax/editor/images"),
        );
        // These tests exercise egui directly, without a display or theme polling.
        app.theme_polled = Instant::now();
        (context, app)
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
        assert!(app.error.as_deref().unwrap().contains("Original artwork"));
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
        assert_eq!(options.preset, Path::new("model.vwm"));
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

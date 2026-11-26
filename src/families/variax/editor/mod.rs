use super::*;
use serde::Deserialize;

mod body_scene;
mod connection;
#[cfg(feature = "usb")]
mod device_sync;
#[cfg(feature = "usb")]
mod follow;
mod guitar;
mod html_export;
mod knobs;
#[cfg(feature = "usb")]
mod live_attachment;
#[cfg(feature = "usb")]
mod live_edit;
mod new_files;
#[cfg(all(feature = "usb", any(target_os = "linux", test)))]
mod procedure_device;
#[cfg(feature = "usb")]
mod refresh;
use new_files::{BankRename, NewFile};
mod slot_files;
mod slot_transfers;
mod startup_library;
#[cfg(all(feature = "usb", any(target_os = "linux", test)))]
mod string_volume;
use slot_files::SlotReplacement;
use slot_transfers::SlotTransfer;
use startup_library::{LibraryImport, LibraryOrigin};
mod pickup_positions;
mod preferences;
mod tuning_presets;
mod twelve_string;
use tuning_presets::TuningDialog;
mod value_format;
use value_format::ValueFormat;

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;

pub(super) const PICKUP1: u32 = 0x02010001;
const PICKUP2: u32 = 0x02010002;
const BODY: u32 = 0x02010003;
const WIRING: u32 = 0x02010004;
const TUNING: u32 = 0x02010005;
const SYSTEM: u32 = variax_workbench::edit::SYSTEM;
const GUITAR_SURFACE: Color32 = Color32::from_rgb(31, 31, 34);
const GUITAR_EDGE: Color32 = Color32::from_rgb(62, 60, 59);
pub(super) const FRET_MARKER: Color32 = Color32::from_rgb(247, 189, 108);

fn guitar_panel() -> egui::Frame {
    egui::Frame::new()
        .inner_margin(12)
        .corner_radius(8)
        .fill(GUITAR_SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, GUITAR_EDGE))
}

#[derive(Clone, Deserialize)]
pub(super) struct ModelArt {
    kind: String,
    id: u32,
    name: String,
    image: String,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub(super) enum Tab {
    Body,
    Pickups,
    Controls,
}

fn load_catalog() -> Vec<ModelArt> {
    serde_json::from_str(include_str!("../../../../assets/model-ui.json"))
        .expect("bundled model catalog")
}

fn model_filename_stem(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Original preference key `variax_editor.auto_select_control_values`.
/// A missing file stays on. `1` is on and `0` is off.
fn load_auto_select(path: &Path) -> bool {
    !matches!(std::fs::read_to_string(path), Ok(text) if text.trim() == "0")
}

fn save_auto_select(path: &Path, enabled: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(if enabled { b"1\n" } else { b"0\n" })
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

struct ParkedLibrary {
    document: EditorDocument,
    path: Option<PathBuf>,
    origin: Option<LibraryOrigin>,
}

pub(super) struct VariaxEditor {
    pub(super) document: Option<EditorDocument>,
    source_path: Option<PathBuf>,
    opening_file: bool,
    parked_library: Option<ParkedLibrary>,
    library_origin: Option<LibraryOrigin>,
    library_import: Option<LibraryImport>,
    connection_status: Option<String>,
    firmware_notice: Option<variax_workbench::firmware_notice::FirmwareNotice>,
    #[cfg(feature = "usb")]
    refresh: refresh::Refresh,
    #[cfg(feature = "usb")]
    follow: follow::Follow,
    #[cfg(feature = "usb")]
    live_edit: live_edit::LiveEdit,
    #[cfg(feature = "usb")]
    device_sync: device_sync::DeviceSync,
    #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
    string_volume: string_volume::Setup,
    catalog: Vec<ModelArt>,
    artwork: PathBuf,
    pub(super) textures: HashMap<String, body_scene::ArtworkTexture>,
    bundled_artwork: body_scene::BundledArtwork,
    missing_textures: HashSet<String>,
    pub(super) error: Option<String>,
    pub(super) status: String,
    pub(super) tab: Tab,
    drag_origin: Option<u32>,
    /// Which pickup is painted last. Pickup 1 starts in front, matching the
    /// original panel's first child. This is view state, not a file field.
    pickup_front: u32,
    /// Unsaved model-name typing. It is not a file field and not an undo step.
    name_draft: Option<String>,
    /// Unsaved author typing. It is not a file field and not an undo step.
    author_draft: Option<String>,
    /// Original `variax_editor.auto_select_control_values`. Default on.
    /// This is an editor preference, not a field in the preset file.
    auto_select_controls: bool,
    /// Where the preference is remembered. Tests leave this empty.
    preferences_path: Option<PathBuf>,
    body_definitions: Option<PathBuf>,
    pickup_position_dialog: Option<pickup_positions::PickupPositionDialog>,
    default_author: String,
    author_preference: Option<preferences::AuthorPreference>,
    link_tuning_mix: bool,
    tuning_dialog: Option<TuningDialog>,
    slot_replacement: Option<SlotReplacement>,
    slot_transfer: Option<SlotTransfer>,
    new_file: Option<NewFile>,
    new_model_collision: Option<PathBuf>,
    bank_rename: Option<BankRename>,
    #[cfg(test)]
    bank_slots: HashMap<usize, Rect>,
    #[cfg(test)]
    bank_options: HashMap<usize, Rect>,
    #[cfg(test)]
    bank_dropdown: Option<Rect>,
    #[cfg(test)]
    pickup_slider: Option<Rect>,
    #[cfg(test)]
    pickup_centers: HashMap<u32, Pos2>,
    #[cfg(test)]
    tuning_toggle: Option<Rect>,
    #[cfg(test)]
    workspace_cards: Option<(Rect, Rect)>,
    #[cfg(test)]
    workspace_tuning: Option<Rect>,
    #[cfg(test)]
    body_art_rect: Option<Rect>,
    #[cfg(test)]
    pickup_controls: HashMap<(u32, &'static str), Rect>,
    #[cfg(test)]
    name_notice: String,
    #[cfg(test)]
    auto_select_checkbox: Option<Rect>,
    #[cfg(test)]
    tuning_widgets: HashMap<&'static str, Rect>,
    #[cfg(test)]
    slot_file_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    slot_transfer_buttons: HashMap<String, Rect>,
    #[cfg(test)]
    new_file_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    html_export_button: Option<Rect>,
    #[cfg(test)]
    tuning_preset_rows: HashMap<String, Rect>,
    #[cfg(test)]
    read_receiver: Option<
        std::sync::mpsc::Receiver<Result<variax_workbench::read_session::CurrentPreset, String>>,
    >,
    #[cfg(test)]
    pub(super) read_worker: Option<(
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    )>,
    pub(super) pending: Option<GuitarPending>,
}

impl VariaxEditor {
    fn new(defaults: &EditorDefaults) -> Self {
        let (artwork, error) = preferences::startup_artwork(defaults);
        Self {
            document: None,
            source_path: None,
            opening_file: false,
            parked_library: None,
            library_origin: None,
            library_import: None,
            connection_status: None,
            firmware_notice: None,
            #[cfg(feature = "usb")]
            refresh: refresh::Refresh::default(),
            #[cfg(feature = "usb")]
            follow: follow::Follow::default(),
            #[cfg(feature = "usb")]
            live_edit: live_edit::LiveEdit::new(defaults.preferences.as_deref()),
            #[cfg(feature = "usb")]
            device_sync: Default::default(),
            #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
            string_volume: Default::default(),
            catalog: load_catalog(),
            artwork,
            textures: HashMap::new(),
            bundled_artwork: body_scene::BundledArtwork::default(),
            missing_textures: HashSet::new(),
            error,
            status: "Edit preset files offline. Your guitar is unchanged.".into(),
            tab: Tab::Body,
            drag_origin: None,
            pickup_front: PICKUP1,
            name_draft: None,
            author_draft: None,
            auto_select_controls: defaults.preferences.as_deref().is_none_or(load_auto_select),
            preferences_path: defaults.preferences.clone(),
            body_definitions: preferences::startup_body_definitions(defaults),
            pickup_position_dialog: None,
            default_author: String::new(),
            author_preference: None,
            link_tuning_mix: false,
            tuning_dialog: None,
            slot_replacement: None,
            slot_transfer: None,
            new_file: None,
            new_model_collision: None,
            bank_rename: None,
            #[cfg(test)]
            bank_slots: HashMap::new(),
            #[cfg(test)]
            bank_options: HashMap::new(),
            #[cfg(test)]
            bank_dropdown: None,
            #[cfg(test)]
            pickup_slider: None,
            #[cfg(test)]
            pickup_centers: HashMap::new(),
            #[cfg(test)]
            tuning_toggle: None,
            #[cfg(test)]
            workspace_cards: None,
            #[cfg(test)]
            workspace_tuning: None,
            #[cfg(test)]
            body_art_rect: None,
            #[cfg(test)]
            pickup_controls: HashMap::new(),
            #[cfg(test)]
            name_notice: String::new(),
            #[cfg(test)]
            auto_select_checkbox: None,
            #[cfg(test)]
            tuning_widgets: HashMap::new(),
            #[cfg(test)]
            slot_file_buttons: HashMap::new(),
            #[cfg(test)]
            slot_transfer_buttons: HashMap::new(),
            #[cfg(test)]
            new_file_buttons: HashMap::new(),
            #[cfg(test)]
            html_export_button: None,
            #[cfg(test)]
            tuning_preset_rows: HashMap::new(),
            #[cfg(test)]
            read_receiver: None,
            #[cfg(test)]
            read_worker: None,
            pending: None,
        }
    }

    pub(super) fn load(&mut self, path: &Path) {
        if self.is_reading() {
            return;
        }
        let result = tabs::open_candidate(path, FAMILY.info().ceiling)
            .and_then(|bytes| EditorDocument::from_file_bytes(bytes, path));
        match result {
            Ok(document) => {
                self.adopt_opened_document(document, Some(path));
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn adopt_opened_document(&mut self, document: EditorDocument, path: Option<&Path>) {
        let parked_library = if document.banks().is_none() {
            if self
                .document
                .as_ref()
                .is_some_and(|current| current.banks().is_some())
            {
                self.document.take().map(|mut library| {
                    if library.is_dirty() {
                        library.discard_changes();
                    }
                    ParkedLibrary {
                        document: library,
                        path: self.source_path.take(),
                        origin: self.library_origin.take(),
                    }
                })
            } else {
                self.parked_library.take()
            }
        } else {
            None
        };
        self.adopt_document(document, path);
        self.parked_library = parked_library;
    }

    fn adopt_document(&mut self, document: EditorDocument, path: Option<&Path>) {
        #[cfg(feature = "usb")]
        {
            self.live_edit.new_document();
            self.device_sync.imported_backup = None;
        }
        self.document = Some(document);
        self.source_path = path.map(Path::to_owned);
        self.parked_library = None;
        self.library_origin = None;
        self.drag_origin = None;
        self.clear_text_drafts();
        self.error = None;
    }

    pub(super) fn dirty(&self) -> bool {
        // Guitar open and read prompts use this. POD tones use the quit guard.
        self.document.as_ref().is_some_and(EditorDocument::is_dirty)
    }

    pub(super) fn is_reading(&self) -> bool {
        #[cfg(test)]
        if self.read_receiver.is_some() {
            return true;
        }
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        if self.string_volume.active() {
            return true;
        }
        #[cfg(feature = "usb")]
        {
            self.refresh.busy() || self.device_sync.busy()
        }
        #[cfg(not(feature = "usb"))]
        {
            false
        }
    }

    pub(super) fn cancel_read(&mut self) {
        #[cfg(feature = "usb")]
        {
            self.refresh.cancel();
            self.device_sync.cancel();
        }
        #[cfg(test)]
        if let Some((cancel, _)) = &self.read_worker {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.status = "Cancelling guitar read…".into();
        }
    }

    fn clear_text_drafts(&mut self) {
        self.name_draft = None;
        self.author_draft = None;
    }

    pub(super) fn undo(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document {
            document.undo();
        } else {
            return;
        }
        self.clear_text_drafts();
    }

    pub(super) fn redo(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document {
            document.redo();
        } else {
            return;
        }
        self.clear_text_drafts();
    }

    #[cfg(test)]
    pub(super) fn start_read(&mut self) {
        // The user has locked all physical-device access during offline validation.
        if hardware_gates::is_locked(Gate::CurrentSettingsRead) {
            self.status = "Live access is locked while offline validation is in progress.".into();
            return;
        }
        if self.is_reading() {
            return;
        }
        if self.dirty() {
            self.pending = Some(GuitarPending::ReadCurrent);
            return;
        }
        self.launch_read();
    }

    #[cfg(test)]
    pub(super) fn launch_read(&mut self) {
        self.status = "Test builds do not open a USB device.".into();
    }

    #[cfg(test)]
    pub(super) fn poll_read(&mut self) {
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

    pub(super) fn artwork_dialog(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        services.request_picker::<Self>(ctx, PickRequest::Artwork, |editor, path, _| {
            if let Some(path) = path {
                editor.choose_artwork(path);
            }
        });
    }

    pub(in crate::editor) fn open_dialog(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if self.dirty() {
            self.pending = Some(GuitarPending::Open(OpenDestination::File));
        } else {
            self.choose_open_file(ctx, services);
        }
    }

    fn choose_open_file(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        if services.request_picker::<Self>(
            ctx,
            PickRequest::OpenWorkbenchFile,
            |editor, path, _| {
                editor.opening_file = false;
                if let Some(path) = path {
                    editor.load(&path);
                }
            },
        ) {
            self.opening_file = true;
        }
    }

    fn save_dialog(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
        continue_after_save: bool,
    ) {
        if self.is_reading() {
            return;
        }
        let Some(document) = &self.document else {
            return;
        };
        let stem = model_filename_stem(
            document
                .selected_preset()
                .map_or("New Collection", PresetFile::name),
        );
        let extension = document.extension();
        let request = PickRequest::SaveWorkbenchFile {
            name: format!("{stem} - edited.{extension}"),
            extension: extension.into(),
        };
        let continuation_ctx = ctx.clone();
        services.request_picker::<Self>(ctx, request, move |editor, path, services| {
            if let Some(path) = path
                && editor.save_copy_to(path)
                && continue_after_save
            {
                editor.continue_pending(&continuation_ctx, services);
            }
        });
    }

    fn save_copy_to(&mut self, path: PathBuf) -> bool {
        let Some(document) = &mut self.document else {
            return false;
        };
        let result = (|| {
            let path = match &self.new_file {
                Some(pending) => pending.destination(&path)?,
                None => path,
            };
            let path = slot_files::with_file_extension(path, document.extension());
            document.save_copy(&path)?;
            document.source = path.clone();
            Ok(path)
        })();
        match result {
            Ok(path) => {
                self.error = None;
                self.source_path = Some(path.clone());
                self.status = format!("Saved {}", path.display());
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    pub(in crate::editor) fn has_document(&self) -> bool {
        self.document.is_some()
    }

    pub(in crate::editor) fn save_file(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
        continue_after_save: bool,
    ) {
        if self.is_reading() {
            return;
        }
        let Some(path) = self.source_path.as_ref() else {
            self.save_dialog(ctx, services, continue_after_save);
            return;
        };
        let Some(document) = &mut self.document else {
            return;
        };
        if let Err(error) = document.save_open_file(path) {
            self.error = Some(error);
            return;
        }
        self.error = None;
        self.status = format!("Saved {}", path.display());
        if continue_after_save {
            self.continue_pending(ctx, services);
        }
    }

    fn return_to_library(&mut self) {
        if let Some(library) = self.parked_library.take() {
            self.adopt_document(library.document, library.path.as_deref());
            self.library_origin = library.origin;
            self.status = "Editing the library.".into();
        }
    }

    pub(super) fn change(&mut self, change: impl FnOnce(&mut PresetFile) -> Result<(), String>) {
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

    pub(super) fn model_id(&self, slot: u32) -> Option<u32> {
        self.document.as_ref()?.preset().model(slot).map(|m| m.id)
    }
    pub(super) fn model_name(&self, slot: u32) -> String {
        self.model_id(slot)
            .and_then(|id| self.catalog.iter().find(|a| a.id == id))
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown model".into())
    }
    pub(super) fn float(&self, slot: u32, id: u32) -> Option<f32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 1).then(|| f32::from_bits(p.bits))
    }
    pub(super) fn integer(&self, slot: u32, id: u32) -> Option<i32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 0).then_some(p.bits as i32)
    }

    fn float_control(
        &mut self,
        ui: &mut egui::Ui,
        slot: u32,
        id: u32,
        label: &str,
        format: ValueFormat,
    ) {
        if let Some(stored) = self.float(slot, id) {
            let mut value = stored;
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
                    .custom_formatter(move |value, _| format.display(value))
                    .custom_parser(move |text| format.parse_edit(text, f64::from(stored)))
                    .update_while_editing(false),
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
            if value.to_bits() != stored.to_bits() {
                self.change(|p| p.set_parameter(slot, id, 1, value.to_bits()));
            }
            if response.drag_stopped()
                && let Some(document) = &mut self.document
            {
                document.end_gesture();
            }
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
        ui.vertical(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt(("electrical", id))
                .selected_text(text)
                .width(ui.available_width())
                .truncate()
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
        ui.vertical(|ui| {
            ui.label("Taper");
            egui::ComboBox::from_id_salt("volume-taper")
                .width(ui.available_width())
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
            let response = ui.add_enabled(
                self.can_edit(slot, None),
                egui::Checkbox::new(&mut enabled, label),
            );
            if response.changed() {
                if matches!(slot, PICKUP1 | PICKUP2) {
                    self.set_pickup_enabled(slot, enabled);
                } else {
                    self.change(|p| p.set_enabled(slot, enabled));
                }
            }
            #[cfg(test)]
            if matches!(slot, PICKUP1 | PICKUP2) {
                self.pickup_controls
                    .insert((slot, "enabled"), response.rect);
            }
        }
    }

    /// Original rule: turning a pickup on brings that pickup forward, and
    /// turning one off brings the other forward. The forced enable, if any,
    /// is inside `set_enabled`, so it shares this edit's one undo step.
    fn set_pickup_enabled(&mut self, slot: u32, enabled: bool) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        let Some(document) = &mut self.document else {
            return;
        };
        if let Err(error) = document.apply(|preset| preset.set_enabled(slot, enabled)) {
            self.error = Some(error);
            return;
        }
        let other = if slot == PICKUP1 { PICKUP2 } else { PICKUP1 };
        self.pickup_front = if enabled { slot } else { other };
    }

    fn front_control(&mut self, ui: &mut egui::Ui, slot: u32) -> egui::Response {
        let which = if slot == PICKUP1 { "1" } else { "2" };
        let response = ui
            .add(
                egui::Button::new("Front")
                    .small()
                    .selected(self.pickup_front == slot),
            )
            .on_hover_text(format!("Move pickup {which} to the front"));
        #[cfg(test)]
        self.pickup_controls.insert((slot, "front"), response.rect);
        response
    }
    fn model_control(&mut self, ui: &mut egui::Ui, slot: u32, kind: &str) {
        let Some(mut id) = self.model_id(slot) else {
            return;
        };
        let previous = id;
        egui::ComboBox::from_id_salt(("model", slot))
            .selected_text(self.model_name(slot))
            .width(ui.available_width())
            .truncate()
            .show_ui(ui, |ui| {
                for model in self.catalog.iter().filter(|a| a.kind == kind) {
                    ui.selectable_value(&mut id, model.id, &model.name);
                }
            });
        if id != previous {
            if matches!(slot, PICKUP1 | PICKUP2) {
                self.select_pickup_model(slot, id);
            } else {
                self.change(|p| p.select_model(slot, id));
            }
        }
    }

    /// A pickup model change and the matching knob values are one undo step.
    /// The original does this from `OnParamChange` only while the preference is on.
    pub(super) fn select_pickup_model(&mut self, slot: u32, id: u32) {
        let auto = self.auto_select_controls;
        self.change(|preset| {
            preset.select_model(slot, id)?;
            if auto {
                preset.apply_pickup_control_values()?;
            }
            Ok(())
        });
    }

    pub(super) fn set_auto_select_controls(&mut self, enabled: bool) {
        self.auto_select_controls = enabled;
        if let Some(path) = self.preferences_path.clone()
            && let Err(error) = save_auto_select(&path, enabled)
        {
            self.error = Some(error);
        }
        if enabled {
            self.change(|preset| preset.apply_pickup_control_values());
        }
    }

    pub(super) fn texture(&mut self, ctx: &egui::Context, relative: &str) -> Option<TextureHandle> {
        self.artwork_texture(ctx, relative)
            .map(|artwork| artwork.handle)
    }

    fn artwork_texture(
        &mut self,
        ctx: &egui::Context,
        relative: &str,
    ) -> Option<body_scene::ArtworkTexture> {
        if let Some(texture) = self.textures.get(relative) {
            return Some(texture.clone());
        }
        if self.missing_textures.contains(relative) {
            return None;
        }
        let path = self.artwork.join(relative);
        match image::open(&path) {
            Ok(image) => {
                let texture = body_scene::ArtworkTexture::new(
                    ctx,
                    relative,
                    image.into_rgba8(),
                    relative.starts_with("pickups/"),
                );
                self.textures.insert(relative.to_owned(), texture.clone());
                Some(texture)
            }
            Err(_) => {
                self.missing_textures.insert(relative.to_owned());
                None
            }
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, height: f32) {
        let slider_width = (ui.available_width() * 0.45).min(190.0);
        let compact = ui.ctx().content_rect().height() < 720.0;
        let style = ui.style_mut();
        style.spacing.item_spacing.y = if compact { 1.0 } else { 3.0 };
        style.spacing.button_padding.y = if compact { 1.0 } else { 2.0 };
        style.spacing.interact_size.y = if compact { 15.0 } else { 17.0 };
        style.spacing.slider_width = slider_width;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (tab, title) in [
                        (Tab::Body, "Body type"),
                        (Tab::Pickups, "Pickups"),
                        (Tab::Controls, "Controls"),
                    ] {
                        let button = egui::Button::selectable(self.tab == tab, title)
                            .min_size(egui::vec2(0.0, variax_workbench::ui::theme::TARGET));
                        if ui.add(button).clicked() {
                            self.tab = tab;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(self.tab)
                    .show(ui, |ui| self.tab_controls(ui));
            },
        );
    }

    fn acoustic_body(&self) -> bool {
        self.model_id(BODY)
            .is_some_and(variax_workbench::edit::is_acoustic)
    }

    fn tab_controls(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        {
            self.pickup_controls.clear();
            self.auto_select_checkbox = None;
        }
        let slot = match self.tab {
            Tab::Body => BODY,
            Tab::Pickups => PICKUP1,
            Tab::Controls => WIRING,
        };
        let availability = self
            .document
            .as_ref()
            .map(|d| d.preset().edit_availability(slot, None));
        let acoustic = self.acoustic_body();
        if acoustic && self.tab == Tab::Pickups {
            ui.strong("No pickups available");
            ui.label("Pickup models are not available when using an acoustic body model.");
        } else if acoustic && self.tab == Tab::Controls {
            ui.strong("Volume knob");
            ui.label("Controls output level.");
            ui.strong("Tone knob");
            ui.label("Changes the modeled microphone position.");
            ui.label("Electrical controls are unavailable for this body.");
            return;
        } else if let Some(Err(reason)) = &availability {
            ui.label("These controls are unavailable for this model.")
                .on_hover_text(reason);
        }
        ui.add_enabled_ui(availability.is_some_and(|a| a.is_ok()), |ui| {
            match self.tab {
                Tab::Body => {
                    ui.strong("Guitar body");
                    self.model_control(ui, BODY, "body");
                    self.body_outline(ui);
                }
                Tab::Pickups => {
                    ui.columns(2, |columns| {
                        self.pickup_column(&mut columns[0], PICKUP1, "Pickup 1");
                        self.pickup_column(&mut columns[1], PICKUP2, "Pickup 2");
                    });
                    ui.horizontal_wrapped(|ui| {
                        self.boolean_control(ui, PICKUP2, 0x200002, "Reverse pickup 2 phase");
                        self.boolean_control(ui, PICKUP2, 0x200004, "Wire pickups in series");
                    });
                }
                Tab::Controls => self.electrical_controls(ui),
            }
        });
    }

    fn pickup_column(&mut self, ui: &mut egui::Ui, slot: u32, title: &str) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(title);
            self.enabled_control(ui, slot, "Enabled");
            if self.front_control(ui, slot).clicked() {
                self.pickup_front = slot;
            }
        });
        self.model_control(ui, slot, "pickup");
        self.standard_pickup_positions(ui, slot);
        if ui.available_width() >= 130.0 {
            ui.spacing_mut().slider_width = (ui.available_width() - 100.0).clamp(30.0, 190.0);
        } else {
            ui.spacing_mut().slider_width = 40.0;
        }
        for (parameter, label, format) in [
            (0x200000, "Position", ValueFormat::POSITION),
            (0x200001, "Angle", ValueFormat::ANGLE),
            (0x200003, "Level", ValueFormat::LEVEL),
        ] {
            ui.label(label);
            self.float_control(ui, slot, parameter, "", format);
        }
    }

    pub(super) fn guitar_workspace(
        &mut self,
        services: &mut EditorServices<'_>,
        ui: &mut egui::Ui,
    ) {
        let viewport = ui.available_rect_before_wrap().intersect(ui.clip_rect());
        if let Some(notice) = self.firmware_notice {
            ui.colored_label(variax_workbench::ui::theme::AMBER, notice.message());
        }
        #[cfg(feature = "usb")]
        if self.device_sync.stopped_after_store() {
            ui.label("Guitar actions are paused after an unverified store. Follow the store recovery instructions before using the guitar again.");
        }
        #[cfg(feature = "usb")]
        self.follow_panel(ui);
        #[cfg(feature = "usb")]
        self.edit_buffer_controls(ui);
        if self.document.is_none() {
            guitar_panel().show(ui, |ui| {
                ui.heading("No Variax library is available");
                ui.label("There is no recorded download or chosen factory firmware file.");
                ui.horizontal_wrapped(|ui| {
                    #[cfg(feature = "usb")]
                    self.download_button(ui);
                    ui.add_enabled_ui(!self.is_reading() && self.pending.is_none(), |ui| {
                        if ui.button("Open file…").clicked() {
                            self.open_dialog(ui.ctx(), services);
                        }
                        if self.preferences_path.is_some()
                            && ui.button("Open library backup…").clicked()
                        {
                            self.choose_library_backup(ui.ctx(), services);
                        }
                        if self.preferences_path.is_some()
                            && ui.button("Choose factory firmware…").clicked()
                        {
                            self.choose_factory_archive(ui.ctx(), services);
                        }
                        self.preference_button(ui);
                    });
                });
            });
            return;
        }
        let available = !self.is_reading() && !self.blocking_prompt();
        if self
            .document
            .as_ref()
            .and_then(EditorDocument::selected_preset)
            .is_none()
        {
            guitar_panel().show(ui, |ui| {
                self.empty_collection(ui, available, services);
                ui.add_enabled_ui(available, |ui| self.preference_button(ui));
            });
            return;
        }
        if self.guitar_header(ui, available) {
            self.choose_library_backup(ui.ctx(), services);
        }
        self.slot_file_actions(services, ui, available);
        let editable = self
            .document
            .as_ref()
            .is_some_and(|document| matches!(document.preset().device_id(), 0x70000 | 0x70004));
        let can_edit = editable && available;
        if !editable {
            ui.label("Editing supports legacy electric Variax presets.");
        }

        use variax_workbench::ui::editor_layout::editor_regions;

        // The bank and model header can grow its UI past the viewport.
        let workspace = ui.available_rect_before_wrap().intersect(viewport);
        let regions = editor_regions(
            workspace,
            self.body_placement().art,
            self.has_twelve_string(),
        );
        let mut body = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("guitar-body")
                .max_rect(regions.body),
        );
        self.guitar(&mut body, regions.body.height(), can_edit);

        let mut inspector = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("guitar-inspector")
                .max_rect(regions.inspector),
        );
        inspector.set_clip_rect(ui.clip_rect().intersect(regions.inspector));
        egui::Frame::new()
            .inner_margin(10)
            .corner_radius(theme::RADIUS_PANEL)
            .fill(theme::PANEL)
            .stroke(egui::Stroke::new(1.0_f32, theme::LINE))
            .show(&mut inspector, |ui| {
                ui.set_min_size(ui.available_size());
                ui.horizontal_wrapped(|ui| {
                    ui.strong(self.model_name(BODY));
                    ui.add_enabled_ui(available, |ui| self.preference_button(ui));
                });
                ui.add_enabled_ui(can_edit, |ui| self.controls(ui, ui.available_height()));
            });

        let mut tuning = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("guitar-tuning")
                .max_rect(regions.tuning),
        );
        tuning.set_clip_rect(ui.clip_rect().intersect(regions.tuning));
        // Short layouts keep the fixed-height string controls reachable within
        // the band without moving the inspector or covering the shell footer.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut tuning, |ui| {
                self.fretboard(ui, can_edit, regions.twelve_string_width)
            });
        #[cfg(test)]
        {
            self.workspace_cards = Some((regions.body, regions.inspector));
            self.workspace_tuning = Some(regions.tuning);
        }
        ui.advance_cursor_after_rect(workspace);
    }

    pub(super) fn variax_pending_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        if self.pending.is_some() || self.new_file.is_some() {
            let save_open =
                matches!(self.pending, Some(GuitarPending::Open(_))) && self.source_path.is_some();
            let modified_positions = match self.pending {
                #[cfg(feature = "usb")]
                Some(GuitarPending::Quit) => self.modified_guitar_positions().len(),
                _ => 0,
            };
            let mut discard = false;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    if modified_positions == 0 {
                        ui.label("Save or discard your changes before continuing.");
                    } else {
                        let positions = if modified_positions == 1 {
                            "position"
                        } else {
                            "positions"
                        };
                        ui.label(format!(
                            "Closing the library discards pending changes at {modified_positions} guitar {positions}."
                        ));
                    }
                    if let Some(error) = &self.error {
                        ui.label(error);
                    }
                    ui.horizontal(|ui| {
                        let response = ui.button(if modified_positions != 0 {
                            "Save a copy…"
                        } else if save_open {
                            "Save and continue"
                        } else {
                            "Save As and continue…"
                        });
                        #[cfg(test)]
                        services.prompt_buttons.insert("save", response.rect);
                        if response.clicked() {
                            save = true;
                        }
                        if save_open {
                            let save_as = ui.button("Save As…");
                            #[cfg(test)]
                            services.prompt_buttons.insert("save-as", save_as.rect);
                            if save_as.clicked() {
                                self.save_dialog(ctx, services, true);
                            }
                        }
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", response.rect);
                        if response.clicked() {
                            cancel = true;
                        }
                        let response = ui.button("Discard changes");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", response.rect);
                        if response.clicked() {
                            discard = true;
                        }
                    });
                });
            if cancel {
                if matches!(self.pending, Some(GuitarPending::Quit)) {
                    services.close = CloseResult::Cancelled;
                }
                self.pending = None;
                self.new_file = None;
                self.library_import = None;
            }
            if save {
                if save_open {
                    self.save_file(ctx, services, true);
                } else {
                    self.save_dialog(ctx, services, modified_positions == 0);
                }
            }
            if discard {
                self.continue_pending(ctx, services);
            }
        }
    }

    fn continue_pending(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        if self.new_file.is_some() {
            self.create_pending_file();
            return;
        }
        match self.pending.take() {
            Some(GuitarPending::Open(OpenDestination::File)) => {
                self.choose_open_file(ctx, services)
            }
            Some(GuitarPending::Open(OpenDestination::Library)) => self.return_to_library(),
            Some(GuitarPending::Open(OpenDestination::Backup)) => self.adopt_imported_library(),
            #[cfg(feature = "usb")]
            Some(GuitarPending::Open(OpenDestination::EditBuffer)) => self.get_edit_buffer(),
            #[cfg(test)]
            Some(GuitarPending::ReadCurrent) => self.launch_read(),
            Some(GuitarPending::Quit) => services.finish_close_if(true),
            None => {}
        }
    }
}

#[cfg(test)]
impl Drop for VariaxEditor {
    fn drop(&mut self) {
        if let Some((cancel, worker)) = self.read_worker.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = worker.join();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GuitarPending {
    Open(OpenDestination),
    #[cfg(test)]
    ReadCurrent,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum OpenDestination {
    File,
    Library,
    Backup,
    #[cfg(feature = "usb")]
    EditBuffer,
}

impl FamilyTab for VariaxEditor {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        self.guitar_workspace(services, ui);
    }
    fn dirty(&self) -> bool {
        #[cfg(feature = "usb")]
        if !self.modified_guitar_positions().is_empty() {
            return true;
        }
        VariaxEditor::dirty(self)
    }
    fn close_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        #[cfg(feature = "usb")]
        {
            self.follow.stop();
            self.live_edit.stop();
        }
        if self.pending.is_none() {
            self.pending = Some(GuitarPending::Quit);
        }
        self.variax_pending_dialog(services, ctx);
    }
    fn status(&self) -> (&str, Option<&str>) {
        (&self.status, self.error.as_deref())
    }
    fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
    fn save_copy(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.save_dialog(ctx, services, false);
    }
    fn undo(&mut self) {
        VariaxEditor::undo(self);
    }
    fn redo(&mut self) {
        VariaxEditor::redo(self);
    }
    fn observe_device(&mut self, device: Option<&crate::editor::tabs::DeviceTab>) {
        self.connection_status = device.map(connection::status);
        self.firmware_notice =
            device.and_then(crate::editor::tabs::DeviceTab::guitar_firmware_notice);
        #[cfg(feature = "usb")]
        {
            self.refresh.observe(device);
            self.follow.observe(device);
            let attachment = device.and_then(live_attachment::Attachment::verified);
            self.live_edit.observe(attachment.clone());
            #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
            self.string_volume.observe(attachment.clone());
            self.device_sync.observe(attachment);
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        #[cfg(feature = "usb")]
        {
            self.poll_device_sync(ctx);
            self.start_automatic_download();
            self.poll_refresh(ctx);
            self.poll_follow(ctx);
            self.poll_edit_buffer(ctx);
        }
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        self.poll_string_volume(ctx);
        #[cfg(test)]
        self.poll_read();
        if self.is_reading() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    #[cfg(feature = "usb")]
    fn take_device_event(&mut self) -> Option<crate::editor::tabs::DeviceEvent> {
        self.follow.take_device_event()
    }
    fn local_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.variax_pending_dialog(services, ctx);
        self.new_model_collision_dialog(ctx, services);
        self.preference_dialog(ctx, services);
        self.tuning_presets_dialog(ctx);
        self.slot_replacement_dialog(services, ctx);
        self.slot_transfer_dialog(services, ctx);
        self.pickup_position_dialog(ctx, services);
        self.rename_bank_dialog(services, ctx);
        #[cfg(feature = "usb")]
        {
            self.refresh_confirmation(ctx);
            self.sync_dialog(ctx);
        }
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        self.string_volume_dialog(ctx);
    }
    fn busy(&self) -> bool {
        #[cfg(feature = "usb")]
        if self.live_edit.busy() {
            return true;
        }
        self.is_reading()
    }
    fn cancel_operation(&mut self) {
        self.cancel_read();
        #[cfg(feature = "usb")]
        self.live_edit.stop();
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        self.string_volume.cancel();
    }
    fn blocking_prompt(&self) -> bool {
        #[cfg(feature = "usb")]
        if self.refresh.confirming() || self.device_sync.prompting() {
            return true;
        }
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        if self.string_volume.active() {
            return true;
        }
        self.pending.is_some()
            || self.tuning_dialog.is_some()
            || self.slot_replacement.is_some()
            || self.slot_transfer.is_some()
            || self.pickup_position_dialog.is_some()
            || self.bank_rename.is_some()
            || self.author_preference.is_some()
            || self.new_file.is_some()
            || self.new_model_collision.is_some()
    }
    fn actions(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        self.new_file_menu(ui, services);
        #[cfg(feature = "usb")]
        self.sync_button(ui);
        #[cfg(all(feature = "usb", any(target_os = "linux", test)))]
        self.string_volume_button(ui);
        #[cfg(feature = "usb")]
        let has_download = {
            let ready = self.download_target_ready();
            if ready && self.document.is_some() {
                self.download_button(ui);
            }
            ready
        };
        #[cfg(not(feature = "usb"))]
        let has_download = false;
        self.connection_controls(ui, services, has_download);
    }
}

pub(super) struct Family;

pub(super) const FAMILY: Family = Family;

impl ProductFamily for Family {
    fn info(&self) -> FamilyInfo {
        FamilyInfo {
            id: "variax",
            title: "Variax Workbench",
            new_label: "Variax",
            extensions: &["vwm", "vwb", "l6c"],
            ceiling: 16_777_216,
        }
    }

    fn claims(&self, bytes: &[u8]) -> bool {
        if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            variax_workbench::bundle::BundleFile::parse(bytes.to_vec()).is_ok_and(|file| {
                let mut patches = file
                    .banks()
                    .iter()
                    .flat_map(|bank| bank.presets())
                    .peekable();
                if patches.peek().is_some() {
                    patches.all(PresetFile::is_legacy_electric)
                } else {
                    file.is_empty_collection()
                }
            })
        } else {
            PresetFile::parse(bytes.to_vec()).is_ok_and(|patch| patch.is_legacy_electric())
        }
    }

    fn open(
        &self,
        defaults: &EditorDefaults,
        source: TabSource,
    ) -> Result<Box<dyn FamilyTab>, String> {
        let mut editor = VariaxEditor::new(defaults);
        match source {
            TabSource::New => editor.load_initial_library(defaults),
            TabSource::File(path, bytes) => {
                editor.adopt_document(EditorDocument::from_file_bytes(bytes, &path)?, Some(&path));
                editor.status = format!("Opened {}", path.display());
            }
        }
        editor.tab = match defaults.startup_panel {
            Some("body") => Tab::Body,
            Some("pickups") => Tab::Pickups,
            Some("controls") => Tab::Controls,
            _ => editor.tab,
        };
        Ok(Box::new(editor))
    }

    fn help(&self, ui: &mut egui::Ui) {
        guitar::milestones(ui);
    }
}

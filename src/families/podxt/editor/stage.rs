use super::*;
use line6_devices::backup::LoadedPodxt;
use line6_devices::family::podxt::{BankKind, EditBuffer, Location, TonePayload};
use pod_preset::{
    ContainerKind, PodControl, PodModelChange, PodTone, UnknownPodDefaults, pod_parameter_table,
};
use variax_workbench::ui::{assets, chrome, controls, knob::Knob, theme};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Block {
    Gate,
    Wah,
    Stomp,
    #[default]
    Amp,
    Comp,
    Eq,
    Vol,
    Mod,
    Delay,
    Reverb,
}

impl Block {
    const MOVABLE: [Self; 4] = [Self::Vol, Self::Mod, Self::Delay, Self::Reverb];

    fn name(self) -> &'static str {
        match self {
            Self::Gate => "Gate",
            Self::Wah => "Wah",
            Self::Stomp => "Stomp",
            Self::Amp => "Amp",
            Self::Comp => "Comp",
            Self::Eq => "EQ",
            Self::Vol => "Volume",
            Self::Mod => "Mod",
            Self::Delay => "Delay",
            Self::Reverb => "Reverb",
        }
    }

    fn group(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Wah => "wah",
            Self::Stomp => "stomp",
            Self::Amp => "amp",
            Self::Comp => "compressor",
            Self::Eq => "eq",
            Self::Vol => "volume pedal",
            Self::Mod => "mod",
            Self::Delay => "delay",
            Self::Reverb => "reverb",
        }
    }

    fn enable(self) -> Option<&'static str> {
        Some(match self {
            Self::Gate => "GATE_ENABLE",
            Self::Wah => "WAH_ENABLE",
            Self::Stomp => "STOMP_ENABLE",
            Self::Amp => "AMP_ENABLE",
            Self::Comp => "COMP_ENABLE",
            Self::Eq => "EQ_ENABLE",
            Self::Vol => return None,
            Self::Mod => "MOD_ENABLE",
            Self::Delay => "DELAY_ENABLE",
            Self::Reverb => "REVERB_ENABLE",
        })
    }

    fn route(self) -> Option<&'static str> {
        match self {
            Self::Vol => Some("VOL_PEDAL_PREPOST"),
            Self::Mod => Some("MOD_PREPOST"),
            Self::Delay => Some("DELAY_PREPOST"),
            Self::Reverb => Some("REVERB_PREPOST"),
            _ => None,
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Gate | Self::Eq => Color32::from_rgb(116, 193, 205),
            Self::Wah | Self::Comp => theme::MUTED,
            Self::Stomp => theme::OK,
            Self::Amp => theme::VARIAX,
            Self::Vol => theme::HD,
            Self::Mod => Color32::from_rgb(153, 161, 238),
            Self::Delay => Color32::from_rgb(234, 151, 187),
            Self::Reverb => Color32::from_rgb(217, 167, 121),
        }
    }

    fn skin(self, model: Option<u32>) -> Option<&'static str> {
        Some(match (self, model) {
            (Self::Wah, Some(33947648)) => "pod-farm/vetta-wah.png",
            (Self::Stomp, Some(33882112)) => "pod-farm/facial-fuzz.png",
            (Self::Mod, Some(33751044)) => "pod-farm/sine-chorus.png",
            (Self::Delay, Some(33685506)) => "pod-farm/analog-delay.png",
            (Self::Reverb, Some(33816578)) => "pod-farm/lux-spring.png",
            (Self::Vol, _) => "pod-farm/volume-pedal.png",
            _ => return None,
        })
    }

    #[cfg(test)]
    fn test_id(self) -> &'static str {
        match self {
            Self::Amp => "pod-block-amp",
            Self::Stomp => "pod-block-stomp",
            Self::Mod => "pod-block-mod",
            Self::Delay => "pod-block-delay",
            Self::Eq => "pod-block-eq",
            Self::Gate => "pod-block-gate",
            Self::Wah => "pod-block-wah",
            Self::Comp => "pod-block-comp",
            Self::Vol => "pod-block-volume",
            Self::Reverb => "pod-block-reverb",
        }
    }
}

const CURRENT_STAGE: usize = usize::MAX;

#[derive(Clone, Copy, Default)]
enum StageSource {
    #[default]
    File,
    Default,
    Backup(Option<Location>),
    DeviceCopy(Location),
    HistoricalCopy(Location),
}

struct StageDocument {
    library: Library,
    bundle: Option<pod_preset::PodBundleLibrary>,
    source: StageSource,
    path: Option<PathBuf>,
}

impl StageDocument {
    fn parse(
        bytes: Vec<u8>,
        source: StageSource,
        path: Option<PathBuf>,
    ) -> Result<Self, pod_preset::ToneError> {
        let mut library = Library::default();
        let bundle = if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            let bundle = pod_preset::PodBundleLibrary::parse(bytes)?;
            for index in 0..bundle.entry_count() {
                let (bank, slot) = bundle.entry_location(index).expect("entry index in range");
                library.import_bytes(bundle.tone(bank, slot)?.into_bytes())?;
            }
            Some(bundle)
        } else {
            library.import_bytes(bytes)?;
            None
        };
        Ok(Self {
            library,
            bundle,
            source,
            path,
        })
    }
}

fn decode_tone(dump: &EditBuffer, kind: BankKind) -> Option<PodTone> {
    let kind = match kind {
        BankKind::Tones => ContainerKind::L6pa,
        BankKind::AmpSetups => ContainerKind::L6as,
        BankKind::FxSetups => ContainerKind::L6fs,
    };
    PodTone::from_podxt_payload(TonePayload::from_dump(dump).ok()?.as_bytes(), kind).ok()
}

#[derive(Default)]
pub(super) struct Stage {
    block: Block,
    connection: Option<(tabs::Connection, detection::DeviceSupport)>,
    art: HashMap<&'static str, assets::UiImage>,
    tap: Option<(usize, Instant)>,
    preferences: Option<PathBuf>,
    backup: Option<LoadedPodxt>,
    source: StageSource,
    pub(super) path: Option<PathBuf>,
    pending_document: Option<StageDocument>,
    bank: usize,
    #[cfg(feature = "usb")]
    pub(super) view_current: bool,
    #[cfg(feature = "usb")]
    library_highlight: Option<Location>,
}

impl Stage {
    pub(super) fn observe(&mut self, device: Option<&tabs::DeviceTab>) {
        let connection = device.map(|tab| (tab.connection, tab.device.support.clone()));
        #[cfg(feature = "usb")]
        if connection != self.connection {
            self.library_highlight = None;
        }
        self.connection = connection;
    }
}

fn ordered_blocks(is_pre: impl Fn(Block) -> bool) -> Vec<Block> {
    let mut ordered = vec![Block::Gate, Block::Wah, Block::Stomp];
    ordered.extend(Block::MOVABLE.into_iter().filter(|&block| is_pre(block)));
    ordered.extend([Block::Amp, Block::Comp, Block::Eq]);
    ordered.extend(Block::MOVABLE.into_iter().filter(|&block| !is_pre(block)));
    ordered
}

fn get<'a>(rows: &'a [PodControl<'static>], key: &str) -> Option<&'a PodControl<'static>> {
    rows.iter().find(|row| row.parameter.key == key)
}

fn choice<'a>(row: &'a PodControl<'_>) -> Option<&'a str> {
    let bits = row.bits?;
    row.choices
        .iter()
        .find(|(value, _)| *value == bits)
        .map(|(_, text)| text.as_ref())
        .or_else(|| {
            row.parameter
                .catalog
                .as_ref()
                .and_then(|_| pod_parameter_table().model_name(bits))
        })
}

fn value(row: &PodControl<'_>) -> Option<f64> {
    row.bits.and_then(|bits| row.displayed(bits))
}

fn label_for_slot(kind: ContainerKind, preset: usize, collection: bool) -> String {
    match (kind, collection) {
        (ContainerKind::L6pa, false) if preset < 128 => {
            format!("{}{}", preset / 4 + 1, ['A', 'B', 'C', 'D'][preset % 4])
        }
        _ => (preset + 1).to_string(),
    }
}

impl PodxtEditor {
    pub(super) fn initialize_stage(&mut self, preferences: Option<PathBuf>) {
        self.stage.backup = preferences
            .as_deref()
            .and_then(pod_preset::latest_podxt_backup);
        self.stage.preferences = preferences;
    }

    fn open_fallback_tone(&mut self) {
        let from_backup = self.stage.backup.as_ref().and_then(|backup| {
            let last = backup
                .after
                .as_ref()
                .or(backup.before.as_ref())
                .or_else(|| backup.entries.first().map(|entry| &entry.tone))?;
            decode_tone(last, BankKind::Tones)
        });
        let (tone, source) = if let Some(tone) = from_backup {
            (tone, StageSource::Backup(None))
        } else {
            (
                PodTone::new_podxt_tone("Default Tone").expect("valid PODxt default"),
                StageSource::Default,
            )
        };
        self.open_stage_copy(tone, source);
        self.pod_message.clear();
    }

    fn open_stage_copy(&mut self, tone: PodTone, source: StageSource) {
        self.request_stage_document(tone.into_bytes(), source, None);
    }

    pub(in crate::editor) fn open_dialog(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        if self.busy() || self.stage_has_pending_document() {
            return;
        }
        services.request_picker::<Self>(ctx, PickRequest::OpenTone, |editor, path, _| {
            let Some(path) = path else {
                return;
            };
            match tabs::open_candidate(&path, pod_preset::BUNDLE_BYTES_CEILING) {
                Ok(bytes) => editor.request_stage_document(bytes, StageSource::File, Some(path)),
                Err(error) => editor.pod_message = error,
            }
        });
    }

    pub(super) fn open_stage_document(&mut self, bytes: Vec<u8>) {
        self.request_stage_document(bytes, StageSource::File, None);
    }

    fn request_stage_document(
        &mut self,
        bytes: Vec<u8>,
        source: StageSource,
        path: Option<PathBuf>,
    ) {
        let document = match StageDocument::parse(bytes, source, path) {
            Ok(document) => document,
            Err(error) => {
                self.pod_message = error.detail();
                return;
            }
        };
        if self.pod_dirty() || self.pod_has_staged_edit() {
            self.stage.pending_document = Some(document);
        } else {
            self.adopt_stage_document(document);
        }
    }

    fn adopt_stage_document(&mut self, document: StageDocument) {
        self.pod_library = document.library;
        self.pod_bundle = document.bundle;
        self.pod_document_generation = self.pod_document_generation.wrapping_add(1);
        self.clear_pod_stages();
        self.parameter_panel.clear_history();
        self.pod_bank = 0;
        self.pod_selected = (!self.pod_library.is_empty()).then_some(0);
        self.refresh_selected_pod_name();
        self.show_file_stage();
        self.stage.source = document.source;
        self.stage.path = document.path;
        #[cfg(feature = "usb")]
        self.live.use_document_directory(self.stage.path.as_deref());
        self.pod_message = self
            .stage
            .path
            .as_ref()
            .map_or_else(String::new, |path| format!("Opened {}", path.display()));
    }

    pub(super) fn stage_has_pending_document(&self) -> bool {
        self.stage.pending_document.is_some()
    }

    pub(super) fn stage_document_prompt(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        if self.stage.pending_document.is_none() {
            return;
        }
        egui::Window::new("Open another PODxt document")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("The current document has unsaved edits.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save copy and open").clicked()
                        && let Some(index) = self.pod_selected
                        && self.write_pod_copy(services, index)
                        && let Some(document) = self.stage.pending_document.take()
                    {
                        self.adopt_stage_document(document);
                    }
                    if ui.button("Discard and open").clicked()
                        && let Some(document) = self.stage.pending_document.take()
                    {
                        self.adopt_stage_document(document);
                    }
                    if ui.button("Cancel").clicked() {
                        self.stage.pending_document = None;
                    }
                });
            });
    }

    pub(super) fn show_stage(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        if self.pod_library.is_empty() && self.pod_bundle.is_none() {
            #[cfg(feature = "usb")]
            let view_current = self.stage.view_current;
            self.open_fallback_tone();
            #[cfg(feature = "usb")]
            {
                self.stage.view_current = view_current;
            }
        }
        #[cfg(test)]
        {
            self.pod_buttons.clear();
            self.pod_rows.clear();
        }
        ui.scope(|ui| {
            ui.set_style(theme::style());
            self.stage_toolbar(ui, services);
            let selected = self.pod_selected.and_then(|index| {
                self.pod_library
                    .tone(index)
                    .cloned()
                    .map(|tone| (index, tone, self.parameter_panel.packs))
            });
            #[cfg(feature = "usb")]
            let selected = if self.stage_is_current() {
                match self.live.stage_tone() {
                    Ok(Some((tone, packs))) => Some((CURRENT_STAGE, tone, packs)),
                    Ok(None) => {
                        self.stage_connection(ui, true);
                        ui.label(
                            self.live
                                .pending_reply_message()
                                .unwrap_or("Reading the playing tone…"),
                        );
                        return;
                    }
                    Err(error) => {
                        self.stage_connection(ui, true);
                        ui.colored_label(theme::TEXT, error);
                        return;
                    }
                }
            } else {
                selected
            };
            if let Some((index, tone, packs)) = selected {
                self.stage_header(ui, index, &tone);
                let rows = tone.table_controls(packs);
                self.stage_chain(ui, index, &rows);
                ui.add_space(theme::SPACE_8);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(185.0);
                        self.stage_side_library(ui, index == CURRENT_STAGE);
                    });
                    ui.vertical(|ui| {
                        ui.set_min_width(ui.available_width());
                        self.stage_editor(ui, index, &tone, &rows);
                        self.stage_settings(ui, index, &rows);
                    });
                });
            } else {
                chrome::section_panel(ui, "PODxt Live", |ui| {
                    controls::readout(ui, "Open a tone or create a new one.");
                });
            }
            if self.pod_revert_confirm {
                ui.label(self.pod_revert_scope());
                ui.horizontal(|ui| {
                    if self.pod_button(ui, "revert-pod-confirm", "Discard edits") {
                        self.revert_pod_edits();
                    }
                    if self.pod_button(ui, "revert-pod-cancel", "Cancel") {
                        self.pod_revert_confirm = false;
                    }
                });
            }
        });
    }

    fn stage_toolbar(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        ui.horizontal_wrapped(|ui| {
            if self.pod_button(ui, "open-stage-document", "Open tone or bank…") {
                self.open_dialog(ui.ctx(), services);
            }
            ui.menu_button("File actions", |ui| {
                if ui.button("Open file editor").clicked() {
                    self.show_file_stage();
                    ui.close();
                }
                if ui.button("Use library backup…").clicked() {
                    services.request_picker::<Self>(
                        ui.ctx(),
                        PickRequest::Folder,
                        |editor, path, _| {
                            let Some(path) = path else {
                                return;
                            };
                            match pod_preset::load_complete_podxt_backup(&path) {
                                Ok(backup) => {
                                    editor.stage.backup = Some(backup);
                                    if let Some(preferences) = &editor.stage.preferences
                                        && let Err(error) =
                                            pod_preset::remember_podxt_backup(preferences, &path)
                                    {
                                        editor.pod_message = error;
                                        return;
                                    }
                                    if !editor.pod_dirty() {
                                        editor.open_fallback_tone();
                                    }
                                }
                                Err(error) => editor.pod_message = error,
                            }
                        },
                    );
                    ui.close();
                }
                self.show_new_documents(ui, services);
                if let Some(sample) = self.practice_tone_menu(ui) {
                    self.open_stage_document(sample.bytes());
                    ui.close();
                }
                if self.pod_button_enabled(
                    ui,
                    "save-pod",
                    "Save copy…",
                    self.pod_selected.is_some(),
                ) {
                    self.save_pod_tone(services);
                    ui.close();
                }
                if self.pod_button_enabled(ui, "revert-pod", "Revert edits…", self.pod_can_revert())
                {
                    self.pod_revert_confirm = true;
                    ui.close();
                }
            });
            let file_actions = !self.stage_is_current();
            if self.pod_button_enabled(
                ui,
                "pod-undo",
                "Undo",
                file_actions && self.parameter_panel.can_history(self.pod_selected, false),
            ) {
                self.table_history(false);
            }
            if self.pod_button_enabled(
                ui,
                "pod-redo",
                "Redo",
                file_actions && self.parameter_panel.can_history(self.pod_selected, true),
            ) {
                self.table_history(true);
            }
            #[cfg(feature = "usb")]
            self.live.toolbar(ui, &mut self.stage.view_current);
            #[cfg(feature = "usb")]
            if self.stage_is_current() {
                self.live.stage_status(ui);
            }
        });
    }

    pub(super) fn show_file_stage(&mut self) {
        self.stage.source = StageSource::File;
        #[cfg(feature = "usb")]
        {
            self.stage.view_current = false;
        }
    }

    pub(super) fn stage_is_current(&self) -> bool {
        #[cfg(feature = "usb")]
        {
            self.stage.view_current && self.live.connected()
        }
        #[cfg(not(feature = "usb"))]
        false
    }

    fn stage_connection(&self, ui: &mut egui::Ui, current: bool) {
        ui.horizontal_wrapped(|ui| {
            match &self.stage.connection {
                None => {
                    chrome::status_chip(ui, chrome::LinkState::NoDevice);
                }
                Some((tabs::Connection::Uncertain, _)) => {
                    chrome::status_chip(ui, chrome::LinkState::Uncertain);
                }
                Some((tabs::Connection::Disconnected, _)) => {
                    chrome::status_chip(ui, chrome::LinkState::Disconnected);
                }
                Some((tabs::Connection::Present, detection::DeviceSupport::Verified(firmware))) => {
                    let status = format!("Connected · firmware {firmware}");
                    #[cfg(feature = "usb")]
                    let status = self
                        .live
                        .playing_channel()
                        .filter(|_| current)
                        .map_or(status.clone(), |ch| {
                            format!("{status} · Channel {}", ch.label())
                        });
                    controls::readout(ui, &status);
                }
                Some((tabs::Connection::Present, detection::DeviceSupport::UsbOnly)) => {
                    controls::readout(ui, "USB detected · identity unverified");
                }
                Some((tabs::Connection::Present, _)) => {
                    chrome::status_chip(ui, chrome::LinkState::IdentityPending);
                }
            }
            let source = if current {
                #[cfg(feature = "usb")]
                if self.live.live_editing {
                    "Live editing · edit buffer"
                } else {
                    "Current tone · Send applies staged edits"
                }
                #[cfg(not(feature = "usb"))]
                "Current tone"
            } else {
                match self.stage.source {
                    StageSource::File => "File editor · nothing is sent",
                    StageSource::Default => "Default tone · offline",
                    StageSource::Backup(_) => "Library backup · offline",
                    StageSource::DeviceCopy(_) => "Device library copy · file only",
                    StageSource::HistoricalCopy(_) => "Previous PODxt library copy · file only",
                }
            };
            controls::readout(ui, source);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                controls::readout(ui, "No live audio");
            });
        });
    }

    fn stage_header(&mut self, ui: &mut egui::Ui, index: usize, tone: &PodTone) {
        self.stage_connection(ui, index == CURRENT_STAGE);
        if index == CURRENT_STAGE {
            ui.horizontal_wrapped(|ui| {
                ui.strong(tone.name());
                #[cfg(feature = "usb")]
                if self
                    .live
                    .playing_channel()
                    .is_some_and(|slot| self.live.channel_modified(slot))
                {
                    ui.label("Unsynchronized");
                }
                if ui.button("Edit a file copy").clicked() {
                    self.open_stage_copy(tone.clone(), StageSource::File);
                }
            });
        } else {
            let name_changed = self.pod_name != tone.name();
            let kind = tone.container();
            let context = self
                .pod_selected
                .and_then(|index| {
                    let bundle = self.pod_bundle.as_ref()?;
                    let (bank, slot) = bundle.entry_location(index)?;
                    Some(format!(
                        "{} · {}",
                        bundle.bank_name(bank)?,
                        label_for_slot(kind, slot, bundle.membership_editable(bank))
                    ))
                })
                .unwrap_or_else(|| match self.stage.source {
                    StageSource::Backup(Some(slot))
                    | StageSource::DeviceCopy(slot)
                    | StageSource::HistoricalCopy(slot) => slot.slot_name(),
                    _ => "No channel · file only".into(),
                });
            ui.horizontal_wrapped(|ui| {
                ui.strong("Tone");
                let response =
                    ui.add(egui::TextEdit::singleline(&mut self.pod_name).desired_width(200.0));
                if (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    || (name_changed && ui.button("Apply name").clicked())
                {
                    self.apply_pod_name();
                }
                controls::readout(ui, &context);
                if self.pod_dirty() {
                    controls::readout(ui, "Unsaved");
                }
            });
        }
        ui.add_space(theme::SPACE_8);
    }

    fn stage_side_library(&mut self, ui: &mut egui::Ui, current: bool) {
        let bank = BankKind::ALL[self.stage.bank];
        let mut rows = Vec::new();
        #[cfg(feature = "usb")]
        let on_device = self.live.connected() && self.live.has_library_bank(bank);
        #[cfg(not(feature = "usb"))]
        let on_device = false;

        #[cfg(feature = "usb")]
        let historical = !on_device
            && self.stage.backup.is_none()
            && Location::all(&[bank]).any(|slot| self.live.historical_tone(slot).is_some());
        #[cfg(not(feature = "usb"))]
        let historical = false;

        for slot in Location::all(&[bank]) {
            #[cfg(feature = "usb")]
            let device = self.live.stored_tone(slot).filter(|_| on_device);
            #[cfg(not(feature = "usb"))]
            let device: Option<&EditBuffer> = None;
            let backup = self
                .stage
                .backup
                .as_ref()
                .filter(|_| !on_device)
                .and_then(|backup| {
                    backup
                        .entries
                        .iter()
                        .find(|entry| entry.location == slot)
                        .map(|entry| &entry.tone)
                });
            #[cfg(feature = "usb")]
            let previous = historical
                .then(|| self.live.historical_tone(slot))
                .flatten();
            #[cfg(not(feature = "usb"))]
            let previous = None;
            if let Some(dump) = device.or(backup).or(previous) {
                rows.push((slot, dump.clone()));
            }
        }
        #[cfg(feature = "usb")]
        let can_show_current = self.live.connected() && self.live.has_current();
        #[cfg(not(feature = "usb"))]
        let can_show_current = false;
        if !current && self.pod_bundle.is_some() {
            #[cfg(feature = "usb")]
            if can_show_current && ui.button("Current POD tone").clicked() {
                self.stage.view_current = true;
            }
            self.stage_library(ui);
            return;
        }
        if rows.is_empty() && self.stage.backup.is_none() && !on_device && !can_show_current {
            self.stage_library(ui);
            return;
        }
        chrome::section_panel(ui, "Library", |ui| {
            ui.set_min_width(153.0);
            ui.set_max_width(185.0);
            if can_show_current && ui.selectable_label(current, "Current POD tone").clicked() {
                #[cfg(feature = "usb")]
                {
                    self.stage.view_current = true;
                }
            }
            ui.weak(if on_device {
                "On PODxt Live"
            } else if self.stage.backup.is_some() {
                "Library backup"
            } else if historical {
                "Previous PODxt library · offline"
            } else {
                "Reading PODxt library"
            });
            egui::ComboBox::from_id_salt("stage-saved-bank")
                .selected_text(bank.name())
                .show_ui(ui, |ui| {
                    for (index, kind) in BankKind::ALL.into_iter().enumerate() {
                        ui.selectable_value(&mut self.stage.bank, index, kind.name());
                    }
                });
            ui.small(match bank {
                BankKind::Tones => "Complete presets on channels 1A–32D.",
                BankKind::AmpSetups => "Recallable amp, cab and mic settings.",
                BankKind::FxSetups => "Recallable effect settings and routing.",
            });
            let mut selected = None;
            let mut file_copy = None;
            egui::ScrollArea::vertical().id_salt("saved-tones").max_height(330.0).show(ui, |ui| {
                for (slot, dump) in &rows {
                    #[cfg(feature = "usb")]
                    let playing = on_device && current && self.live.playing_channel() == Some(*slot);
                    #[cfg(not(feature = "usb"))]
                    let playing = false;
                    let browsing = matches!(
                        self.stage.source, StageSource::Backup(Some(ch)) | StageSource::DeviceCopy(ch) | StageSource::HistoricalCopy(ch) if ch == *slot
                    ) && !current;
                    #[cfg(feature = "usb")]
                    let unsynchronized = on_device && self.live.channel_modified(*slot);
                    #[cfg(not(feature = "usb"))]
                    let unsynchronized = false;
                    let unsaved = browsing && self.pod_selected.is_some_and(|index| self.pod_library.is_dirty(index));
                    let suffix = if unsaved || unsynchronized { " !" } else { "" };
                    let response = ui.add(
                        egui::Button::selectable(playing || browsing, format!("{}{suffix}  {}", slot.slot_name(), dump.name())).truncate()
                    ).on_hover_text(if unsynchronized {
                        "Unsynchronized with the saved POD slot"
                    } else if unsaved {
                        "Unsaved file edits"
                    } else if historical {
                        "Open a copy of the previous connection’s tone."
                    } else if on_device && current {
                        "Enable Live editing to recall a channel, or right-click to open a file copy."
                    } else { "" });
                    #[cfg(feature = "usb")]
                    if playing && self.stage.library_highlight != Some(*slot) {
                        response.scroll_to_me(Some(egui::Align::Center));
                        self.stage.library_highlight = Some(*slot);
                    }
                    response.context_menu(|ui| {
                        if ui.button("Open as file copy").clicked() {
                            file_copy = Some((*slot, dump));
                            ui.close();
                        }
                    });
                    if response.clicked() {
                        selected = Some((*slot, dump));
                    }
                }
            });
            #[cfg(feature = "usb")]
            if let Some((slot, _)) = selected
                && on_device
                && self.live.live_editing
                && slot.kind() == BankKind::Tones
            {
                self.live.recall(slot, ui.ctx());
                self.stage.view_current = true;
                return;
            }
            if let Some((slot, dump)) =
                file_copy.or_else(|| (!current || historical).then_some(selected).flatten())
            {
                if let Some(tone) = decode_tone(dump, slot.kind()) {
                    self.open_stage_copy(
                        tone,
                        if on_device {
                            StageSource::DeviceCopy(slot)
                        } else if historical {
                            StageSource::HistoricalCopy(slot)
                        } else {
                            StageSource::Backup(Some(slot))
                        },
                    );
                }
            } else if selected.is_some() {
                self.pod_message =
                    "Live editing is off. Right-click a channel to open a file copy.".into();
            }
        });
    }

    fn stage_library(&mut self, ui: &mut egui::Ui) {
        chrome::section_panel(ui, "Library", |ui| {
            ui.set_min_width(153.0);
            ui.set_max_width(185.0);
            let names: Vec<_> = self.pod_bundle.as_ref().map_or_else(Vec::new, |bundle| {
                (0..bundle.bank_count())
                    .map(|bank| bundle.bank_name(bank).unwrap_or("Bank"))
                    .collect()
            });
            let mut bank = Some(self.pod_bank.min(names.len().saturating_sub(1)));
            if names.len() > 1 {
                ui.add(controls::Selector::new(
                    ui.id().with("banks"),
                    &mut bank,
                    &names,
                    "Bank",
                ));
                if let Some(bank) = bank.filter(|&bank| bank != self.pod_bank) {
                    let slot = self
                        .pod_selected_channel()
                        .unwrap_or(0)
                        .min(self.pod_channel_count(bank).saturating_sub(1));
                    self.show_pod_channel(bank, slot);
                    self.pod_bank = bank;
                }
            }
            let rows: Vec<_> = (0..self.pod_library.len())
                .filter_map(|index| {
                    let tone = self.pod_library.tone(index)?;
                    let label = if let Some(bundle) = &self.pod_bundle {
                        let (bank, slot) = bundle.entry_location(index)?;
                        if bank != self.pod_bank {
                            return None;
                        }
                        format!(
                            "{}  {}",
                            label_for_slot(
                                tone.container(),
                                slot,
                                bundle.membership_editable(bank)
                            ),
                            tone.name()
                        )
                    } else {
                        tone.name().to_owned()
                    };
                    Some((
                        index,
                        if self.pod_library.is_dirty(index) {
                            format!("!  {label}")
                        } else {
                            label
                        },
                    ))
                })
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("stage-library")
                .max_height(302.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (index, label) in &rows {
                        let response = ui.add(
                            egui::Button::selectable(self.pod_selected == Some(*index), label)
                                .truncate(),
                        );
                        #[cfg(test)]
                        self.pod_rows.insert(*index, response.rect);
                        if response.clicked() {
                            self.select_pod_index(*index);
                        }
                    }
                });
            if let Some(index) = self.pod_selected
                && let Some(bundle) = &self.pod_bundle
                && let Some((bank, slot)) = bundle.entry_location(index)
            {
                let flexible = bundle.membership_editable(bank);
                if flexible || bundle.fixed_bank_reorder_editable(bank) {
                    let count = bundle.preset_count(bank).unwrap_or(0);
                    ui.menu_button("Manage entry", |ui| {
                        for (name, action, available) in [
                            ("Duplicate", PodMembershipAction::Duplicate, flexible),
                            ("Remove", PodMembershipAction::Remove, flexible),
                            ("Move up", PodMembershipAction::MoveUp, slot > 0),
                            ("Move down", PodMembershipAction::MoveDown, slot + 1 < count),
                        ] {
                            if ui.add_enabled(available, egui::Button::new(name)).clicked() {
                                self.apply_pod_membership_action(action, bank, slot);
                                ui.close();
                            }
                        }
                    });
                }
            }
        });
    }

    fn stage_chain(&mut self, ui: &mut egui::Ui, index: usize, rows: &[PodControl<'static>]) {
        let is_pre = |block: Block| {
            block
                .route()
                .and_then(|key| get(rows, key))
                .and_then(|row| row.bits)
                == Some(0)
        };
        let chain = ordered_blocks(is_pre);
        let root = assets::workspace_root();
        let tiles: Vec<_> = chain
            .into_iter()
            .map(|block| {
                let model = rows.iter().find(|row| {
                    row.parameter.block == block.group() && row.parameter.catalog.is_some()
                });
                let skin = block.skin(model.and_then(|row| row.bits));
                if let Some(path) = skin {
                    self.stage
                        .art
                        .entry(path)
                        .or_insert_with(|| assets::load(ui.ctx(), &root, path));
                }
                (block, model.and_then(choice), skin)
            })
            .collect();
        let image_width = |skin: Option<&str>| {
            skin.and_then(|path| self.stage.art.get(path))
                .map_or(chrome::STOMP_SLOT.x, |image| {
                    chrome::chain_slot(image.pixels).x
                })
        };
        let available = ui.available_width();
        let gap = 5.0;
        let weights: Vec<_> = tiles
            .iter()
            .map(|&(_, _, skin)| {
                if image_width(skin) > chrome::STOMP_SLOT.x {
                    1.46
                } else {
                    1.0
                }
            })
            .collect();
        let usable = available - gap * (tiles.len() - 1) as f32;
        let unit = usable / weights.iter().sum::<f32>();
        let (area, _) = ui.allocate_exact_size(egui::vec2(available, 110.0), Sense::hover());
        let stage = self
            .stage
            .art
            .entry("pod-farm/stage-frame.png")
            .or_insert_with(|| assets::load(ui.ctx(), &root, "pod-farm/stage-frame.png"));
        ui.painter()
            .rect_filled(area.expand(2.0), theme::RADIUS_PANEL, theme::BG);
        if let Some(texture) = &stage.texture {
            ui.painter().image(
                texture.id(),
                area,
                Rect::from_min_max(egui::pos2(0.0, 0.12), egui::pos2(1.0, 0.35)),
                Color32::from_gray(105),
            );
        }
        let mut x = area.left();
        for ((block, model, skin), weight) in tiles.into_iter().zip(weights) {
            let width = unit * weight;
            let bounds =
                Rect::from_min_size(egui::pos2(x, area.top()), egui::vec2(width, area.height()));
            x += width + gap;
            let enable = block.enable().and_then(|key| get(rows, key));
            let on = enable
                .and_then(|row| row.bits)
                .is_some_and(|bits| bits != 0)
                || block == Block::Vol;
            let placement = if block.route().is_some() || block == Block::Eq {
                Some(if is_pre(block) { "Pre" } else { "Post" })
            } else {
                None
            };
            let tile = Tile {
                block,
                model,
                placement,
                selected: block == self.stage.block,
                on,
                image: skin.and_then(|path| self.stage.art.get(path)),
            };
            let route = block
                .route()
                .and_then(|key| get(rows, key))
                .filter(|row| row.bits.is_some());
            let can_edit = |row: Option<&PodControl<'_>>| {
                row.is_some_and(|row| self.stage_control_enabled(index, row))
            };
            let interaction = tile.show(ui, bounds, can_edit(enable), can_edit(route));
            #[cfg(test)]
            self.pod_buttons
                .insert(block.test_id(), interaction.select.rect);
            if interaction.select.clicked() {
                self.stage.block = block;
            }
            if interaction.toggle
                && let Some(row) = enable
            {
                self.edit_stage_value_with_ui(ui, index, row, u32::from(!on));
            }
            if interaction.reorder
                && let Some(row) = route
            {
                self.edit_stage_value_with_ui(ui, index, row, u32::from(is_pre(block)));
            }
        }
    }

    fn stage_editor(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        tone: &PodTone,
        rows: &[PodControl<'static>],
    ) {
        let block = self.stage.block;
        let model = rows
            .iter()
            .find(|row| row.parameter.block == block.group() && row.parameter.catalog.is_some());
        let title = match block {
            Block::Eq => "EQ · four bands".to_owned(),
            Block::Amp => format!("Amp · {}", model.and_then(choice).unwrap_or("No amp model")),
            _ => block.name().to_owned(),
        };
        chrome::section_panel(ui, &title, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(theme::SPACE_8);
            match block {
                Block::Amp => self.stage_amp(ui, index, rows),
                Block::Eq => self.stage_eq(ui, index, tone, rows),
                _ => {
                    let selected: Vec<_> = rows
                        .iter()
                        .filter(|row| {
                            row.parameter.block == block.group()
                                && Some(row.parameter.key.as_str()) != block.enable()
                        })
                        .collect();
                    if selected.is_empty() {
                        controls::readout(ui, "No controls stored for this block.");
                    }
                    ui.horizontal_wrapped(|ui| {
                        for row in selected {
                            self.stage_control(ui, index, row);
                        }
                    });
                }
            }
        });
    }

    fn stage_amp(&mut self, ui: &mut egui::Ui, index: usize, rows: &[PodControl<'static>]) {
        egui::Frame::new()
            .fill(theme::BAR)
            .stroke(egui::Stroke::new(1.0_f32, theme::LINE))
            .corner_radius(theme::RADIUS_CONTROL)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    for key in ["AMP_MODEL", "CAB_MODEL", "MIC_MODEL"] {
                        if let Some(row) = get(rows, key) {
                            self.stage_control(ui, index, row);
                        }
                    }
                });
            });
        ui.add_space(theme::SPACE_12);
        let keys = ["DRIVE", "BASS", "MID", "TREBLE", "PRESENCE", "CHANVOL"];
        ui.columns(keys.len(), |columns| {
            for (ui, key) in columns.iter_mut().zip(keys) {
                if let Some(row) = get(rows, key) {
                    self.stage_control(ui, index, row);
                }
            }
        });
        if let Some(row) = get(rows, "ROOM_LEVEL") {
            ui.collapsing("A.I.R. settings", |ui| self.stage_control(ui, index, row));
        }
    }

    fn stage_eq(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        tone: &PodTone,
        rows: &[PodControl<'static>],
    ) {
        let bands = ["EQ_FREQ1", "EQ_FREQ2", "EQ_FREQ3", "EQ_FREQ4"]
            .into_iter()
            .zip(["EQ_GAIN1", "EQ_GAIN2", "EQ_GAIN3", "EQ_GAIN4"]);
        eq_graph(
            ui,
            &bands
                .clone()
                .filter_map(|(frequency, gain)| {
                    Some((value(get(rows, frequency)?)?, value(get(rows, gain)?)?))
                })
                .collect::<Vec<_>>(),
        );
        ui.add_space(theme::SPACE_8);
        ui.columns(4, |columns| {
            for (band, (ui, (frequency, gain))) in columns.iter_mut().zip(bands).enumerate() {
                ui.strong(format!("Band {}", band + 1));
                let layout = if ui.available_width() >= 156.0 {
                    egui::Layout::left_to_right(egui::Align::Min)
                } else {
                    egui::Layout::top_down(egui::Align::Min)
                };
                ui.with_layout(layout, |ui| {
                    for key in [frequency, gain] {
                        if let Some(row) = get(rows, key) {
                            self.stage_control(ui, index, row);
                        }
                    }
                });
            }
        });
        if chrome::toolbar_button(
            ui,
            "Reset EQ",
            index != CURRENT_STAGE && tone.podxt_eq_reset_context().is_ok(),
            "The stored EQ is unavailable.",
        )
        .clicked()
            && let Err(error) = self.table_edit(index, |tone| {
                tone.reset_podxt_eq()?;
                Ok(PodModelChange::default())
            })
        {
            self.pod_message = error.detail();
        }
    }

    fn edit_stage_value_with_ui(
        &mut self,
        ui: &egui::Ui,
        index: usize,
        row: &PodControl<'_>,
        bits: u32,
    ) {
        if index == CURRENT_STAGE {
            #[cfg(feature = "usb")]
            if let Some(target) = row.live_target(bits) {
                self.live.queue_stage_target(target, ui.ctx());
            }
            return;
        }
        let _ = ui;
        self.edit_stage_value(index, row, bits);
    }

    fn edit_stage_value(&mut self, index: usize, row: &PodControl<'_>, bits: u32) {
        if row.bits == Some(bits) {
            return;
        }
        let key = row.parameter.key.as_str();
        let packs = self.parameter_panel.packs;
        let defaults = !self.parameter_panel.keep_amp_settings;
        let result = self.table_edit(index, |tone| {
            if row.parameter.catalog.is_some() {
                tone.set_table_model(key, bits, packs, defaults, &UnknownPodDefaults)
            } else {
                tone.set_table_parameter(key, bits)?;
                Ok(PodModelChange {
                    affected: row.parameter.selector.into_iter().collect(),
                    ..Default::default()
                })
            }
        });
        if let Err(error) = result {
            self.pod_message = error.detail();
        }
    }

    fn stage_control_enabled(&self, index: usize, _row: &PodControl<'_>) -> bool {
        if index != CURRENT_STAGE {
            return true;
        }
        #[cfg(feature = "usb")]
        {
            _row.live_control()
                .is_some_and(|control| self.live.can_change_control(control, &_row.parameter.block))
        }
        #[cfg(not(feature = "usb"))]
        false
    }

    fn stage_control(&mut self, ui: &mut egui::Ui, index: usize, row: &PodControl<'_>) {
        let selector = row.parameter.selector.expect("editable table control");
        let label = match row.parameter.key.as_str() {
            "AMP_MODEL" => "Amp model",
            "CAB_MODEL" => "Cab",
            "MIC_MODEL" => "Mic",
            "DRIVE" => "Drive",
            "BASS" => "Bass",
            "MID" => "Mid",
            "TREBLE" => "Treble",
            "PRESENCE" => "Presence",
            "CHANVOL" => "Volume",
            "VARIAX_FAMILY" => "Guitar family",
            "TWEAK_PARAM" => "Tweak",
            "EQ_FREQ1" | "EQ_FREQ2" | "EQ_FREQ3" | "EQ_FREQ4" => "Frequency",
            "EQ_GAIN1" | "EQ_GAIN2" | "EQ_GAIN3" | "EQ_GAIN4" => "Gain",
            _ => row.label(),
        };
        let unknown = self
            .parameter_panel
            .unknown
            .get(&index)
            .is_some_and(|rows| rows.contains(&selector));
        let enabled = self.stage_control_enabled(index, row);
        let bits = ui
            .push_id((index, selector), |ui| {
                ui.vertical(|ui| {
                    let bits = if !row.choices.is_empty() {
                        ui.label(label);
                        ui.add_enabled_ui(enabled, |ui| stage_selector(ui, row))
                            .inner
                    } else if let (Some([min, max]), Some(format)) =
                        (row.display_range(), row.format)
                        && max > min
                    {
                        let draft = self
                            .parameter_panel
                            .slider
                            .filter(|&(tone, control, _)| tone == index && control == selector);
                        let steady = row
                            .bits
                            .or_else(|| row.neutral_bits())
                            .and_then(|bits| row.displayed(bits))
                            .unwrap_or(min);
                        let mut current = draft.map_or(steady, |(_, _, value)| value);
                        let suffix = format.unit.as_deref().unwrap_or("");
                        let output = Knob::percent(label, &mut current)
                            .range(min..=max)
                            .decimals(format.decimals.unwrap_or(0).max(0) as usize)
                            .suffix(suffix)
                            .enabled(enabled)
                            .show(ui);
                        if output.response.dragged() || output.response.drag_started() {
                            self.parameter_panel.slider = Some((index, selector, current));
                            None
                        } else if output.response.drag_stopped() || output.changed {
                            self.parameter_panel.slider = None;
                            row.bits_from_display(current)
                        } else {
                            None
                        }
                    } else {
                        ui.label(label);
                        None
                    };
                    #[cfg(feature = "usb")]
                    if index == CURRENT_STAGE && row.live_control().is_none() {
                        ui.small("File only");
                    }
                    if unknown {
                        ui.small("default unknown");
                    } else if row.bits.is_none() && row.parameter.catalog.is_none() {
                        ui.weak("Not stored");
                    }
                    bits
                })
                .inner
            })
            .inner;
        if let Some(bits) = bits {
            self.edit_stage_value_with_ui(ui, index, row, bits);
        }
    }

    fn stage_settings(&mut self, ui: &mut egui::Ui, index: usize, rows: &[PodControl<'static>]) {
        ui.horizontal_wrapped(|ui| {
            if let Some(row) = get(rows, "TEMPO") {
                if let Some(bpm) = value(row) {
                    controls::readout(ui, &format!("{bpm:.1} BPM"));
                }
                if chrome::toolbar_button(ui, "Tap", self.stage_control_enabled(index, row), "")
                    .on_hover_text("Set tone tempo")
                    .clicked()
                {
                    let now = Instant::now();
                    let prior = self.stage.tap.replace((index, now));
                    if let Some((_, prior)) = prior.filter(|(prior_index, _)| *prior_index == index)
                        && let Some(bits) =
                            row.bits_from_display(60.0 / now.duration_since(prior).as_secs_f64())
                    {
                        self.edit_stage_value_with_ui(ui, index, row, bits);
                    }
                }
            }
            controls::readout(ui, linked_label(rows));
            if let Some(row) = get(rows, "TWEAK_PARAM") {
                ui.label("Tweak");
                let enabled = self.stage_control_enabled(index, row);
                if let Some(bits) = ui
                    .add_enabled_ui(enabled, |ui| stage_selector(ui, row))
                    .inner
                {
                    self.edit_stage_value_with_ui(ui, index, row, bits);
                }
            }
        });
        ui.collapsing("Tone settings", |ui| {
            if index != CURRENT_STAGE {
                ui.horizontal_wrapped(|ui| {
                    let mut amp_defaults = !self.parameter_panel.keep_amp_settings;
                    if ui
                        .checkbox(&mut amp_defaults, "Load amp defaults")
                        .on_hover_text(
                            "Shared controls keep their settings until defaults are available.",
                        )
                        .changed()
                    {
                        self.parameter_panel.keep_amp_settings = !amp_defaults;
                    }
                    ui.menu_button("Model packs", |ui| {
                        for (mask, name) in [
                            (1, "Metal Shop"),
                            (2, "Collector Classics"),
                            (4, "FX Junkie"),
                            (8, "Bass Expansion"),
                        ] {
                            let mut set = self.parameter_panel.packs & mask != 0;
                            if ui.checkbox(&mut set, name).changed() {
                                self.parameter_panel.packs ^= mask;
                            }
                        }
                    });
                });
            }
            let family = get(rows, "VARIAX_FAMILY").and_then(|row| row.bits);
            ui.horizontal_wrapped(|ui| {
                for row in rows.iter().filter(|row| {
                    matches!(
                        row.parameter.block.as_str(),
                        "system" | "tempo" | "Variax link"
                    ) && row
                        .parameter
                        .variax_family
                        .is_none_or(|value| Some(value) == family)
                }) {
                    self.stage_control(ui, index, row);
                }
            });
        });
        if index != CURRENT_STAGE {
            self.metadata_panel(ui, index);
        }
    }
}

fn linked_label<'a>(rows: &'a [PodControl<'static>]) -> &'a str {
    match get(rows, "VARIAX_FAMILY").and_then(|row| row.bits) {
        None | Some(127) => "No linked model",
        Some(family) => rows
            .iter()
            .find(|row| {
                row.parameter.variax_family == Some(family) && row.parameter.key.ends_with("_MODEL")
            })
            .and_then(choice)
            .unwrap_or("Linked model unavailable"),
    }
}

fn stage_selector(ui: &mut egui::Ui, row: &PodControl<'_>) -> Option<u32> {
    let choices: Vec<_> = row.choices.iter().map(|(_, text)| text.as_ref()).collect();
    let starting = row
        .choices
        .iter()
        .position(|(bits, _)| Some(*bits) == row.bits);
    let mut selected = starting;
    ui.add(
        controls::Selector::new(
            ui.id().with(("choice", &row.parameter.key)),
            &mut selected,
            &choices,
            choice(row).unwrap_or("Select…"),
        )
        .enabled(!choices.is_empty()),
    );
    selected
        .filter(|&selected| Some(selected) != starting)
        .map(|selected| row.choices[selected].0)
}

struct Tile<'a> {
    block: Block,
    model: Option<&'a str>,
    placement: Option<&'a str>,
    selected: bool,
    on: bool,
    image: Option<&'a assets::UiImage>,
}

struct TileResponse {
    select: egui::Response,
    toggle: bool,
    reorder: bool,
}

impl Tile<'_> {
    fn show(&self, ui: &mut egui::Ui, rect: Rect, can_toggle: bool, movable: bool) -> TileResponse {
        let id = ui.make_persistent_id(("signal-chain", self.block.name()));
        let response = ui.interact(rect, id, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                true,
                self.selected,
                self.block.name(),
            )
        });
        let lamp = rect.left_top() + egui::vec2(11.0, 15.0);
        let lamp_clicked = if can_toggle {
            let switch = ui
                .interact(
                    Rect::from_center_size(lamp, egui::vec2(22.0, 30.0)),
                    id.with("bypass"),
                    Sense::click(),
                )
                .on_hover_text(if self.on {
                    "Bypass block"
                } else {
                    "Enable block"
                });
            switch.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Checkbox,
                    true,
                    self.on,
                    format!("{} enabled", self.block.name()),
                )
            });
            switch.clicked()
        } else {
            false
        };
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
        let accent = self.block.color();
        let fill = if self.selected || response.hovered() {
            theme::RAISED
        } else {
            theme::PANEL
        };
        let outline = if self.selected || response.has_focus() {
            theme::AMBER
        } else {
            theme::LINE
        };
        painter.rect(
            rect,
            theme::RADIUS_PANEL,
            fill,
            egui::Stroke::new(if self.selected { 2.0_f32 } else { 1.0_f32 }, outline),
            egui::StrokeKind::Inside,
        );
        painter.line_segment(
            [
                rect.left_top() + egui::vec2(7.0, 3.0),
                rect.right_top() + egui::vec2(-7.0, 3.0),
            ],
            egui::Stroke::new(2.0_f32, accent),
        );
        controls::paint_led(&painter, lamp, self.on, true);
        painter.text(
            lamp + egui::vec2(11.0, 0.0),
            egui::Align2::LEFT_CENTER,
            self.block.name(),
            egui::FontId::proportional(theme::COMPACT),
            theme::TEXT,
        );
        let artwork = Rect::from_min_max(
            rect.left_top() + egui::vec2(5.0, 29.0),
            rect.right_bottom() - egui::vec2(5.0, 24.0),
        );
        if let Some(art) = self.image.filter(|image| !image.missing) {
            assets::paint_tinted(
                ui,
                art,
                artwork,
                if self.on {
                    Color32::WHITE
                } else {
                    Color32::from_gray(145)
                },
            );
        } else {
            glyph(
                &painter,
                artwork,
                self.block,
                if self.on {
                    accent
                } else {
                    accent.gamma_multiply(0.57)
                },
            );
        }
        let route_width = if self.placement.is_some() { 31.0 } else { 0.0 };
        let caption_area = Rect::from_min_max(
            rect.left_bottom() + egui::vec2(5.0, -23.0),
            rect.right_bottom() - egui::vec2(5.0 + route_width, 4.0),
        );
        if let Some(model) = self.model {
            let mut job = egui::text::LayoutJob::single_section(
                model.to_owned(),
                egui::TextFormat {
                    font_id: egui::FontId::proportional(theme::KNOB_LABEL),
                    color: theme::MUTED,
                    ..Default::default()
                },
            );
            job.wrap.max_width = caption_area.width();
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let galley = painter.layout_job(job);
            let start = caption_area.center() - galley.size() / 2.0;
            painter.galley(start, galley, theme::MUTED);
        }
        let mut reorder = false;
        if let Some(placement) = self.placement {
            let badge = Rect::from_min_max(
                rect.right_bottom() - egui::vec2(35.0, 24.0),
                rect.right_bottom() - egui::vec2(4.0, 4.0),
            );
            if movable {
                let label = if placement == "Pre" {
                    "Move after the amp"
                } else {
                    "Move before the amp"
                };
                let route = ui
                    .interact(badge, id.with("placement"), Sense::click())
                    .on_hover_text(label);
                route.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                });
                painter.rect_filled(
                    badge,
                    theme::RADIUS_CONTROL,
                    if route.hovered() || route.has_focus() {
                        theme::LINE
                    } else {
                        theme::BAR
                    },
                );
                reorder = route.clicked();
            }
            painter.text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                placement,
                egui::FontId::proportional(theme::KNOB_LABEL),
                if movable { accent } else { theme::MUTED },
            );
        }
        TileResponse {
            select: response.on_hover_text(self.model.unwrap_or(self.block.name())),
            toggle: lamp_clicked,
            reorder,
        }
    }
}

fn glyph(painter: &egui::Painter, bounds: Rect, block: Block, color: Color32) {
    let center = bounds.center();
    let size = egui::vec2(bounds.width().min(56.0), 27.0);
    let plate = Rect::from_center_size(center, size);
    let stroke = egui::Stroke::new(1.5_f32, color);
    painter.rect_stroke(
        plate,
        theme::RADIUS_CONTROL,
        stroke,
        egui::StrokeKind::Inside,
    );
    match block {
        Block::Gate => {
            painter.add(egui::Shape::line(
                vec![
                    center + egui::vec2(-16.0, 5.0),
                    center + egui::vec2(-16.0, -6.0),
                    center + egui::vec2(-6.0, -6.0),
                    center + egui::vec2(6.0, 5.0),
                    center + egui::vec2(16.0, 5.0),
                ],
                stroke,
            ));
        }
        Block::Eq => {
            for (x, height) in [(-15.0, 8.0), (-5.0, 13.0), (5.0, 10.0), (15.0, 16.0)] {
                painter.line_segment(
                    [
                        center + egui::vec2(x, 8.0),
                        center + egui::vec2(x, 8.0 - height),
                    ],
                    stroke,
                );
            }
        }
        Block::Comp => {
            for x in [-9.0, 9.0] {
                painter.circle_stroke(center + egui::vec2(x, 0.0), 6.0, stroke);
            }
        }
        Block::Amp => {
            for y in [-5.0, 0.0, 5.0] {
                painter.hline(center.x - 16.0..=center.x + 5.0, center.y + y, stroke);
            }
            painter.circle_filled(center + egui::vec2(12.0, 0.0), 2.5, color);
        }
        _ => {
            painter.line_segment(
                [
                    center + egui::vec2(-12.0, 5.0),
                    center + egui::vec2(12.0, -5.0),
                ],
                stroke,
            );
        }
    }
}

fn eq_graph(ui: &mut egui::Ui, bands: &[(f64, f64)]) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 110.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, theme::RADIUS_CONTROL, theme::BAR);
    let plot = rect.shrink2(egui::vec2(18.0, 14.0));
    let hz = |freq: f64| {
        plot.left() + (freq.clamp(20.0, 20_000.0) / 20.0).log10() as f32 / 3.0 * plot.width()
    };
    let db = |gain: f64| plot.center().y - (gain.clamp(-24.0, 24.0) as f32 / 48.0) * plot.height();
    painter.hline(
        plot.x_range(),
        plot.center().y,
        egui::Stroke::new(1.0_f32, theme::HD),
    );
    for (frequency, caption) in [(100.0, "100 Hz"), (1000.0, "1 kHz"), (10000.0, "10 kHz")] {
        let x = hz(frequency);
        painter.vline(x, plot.y_range(), egui::Stroke::new(1.0_f32, theme::LINE));
        painter.text(
            egui::pos2(x + 3.0, plot.bottom()),
            egui::Align2::LEFT_BOTTOM,
            caption,
            egui::FontId::proportional(theme::KNOB_LABEL),
            theme::MUTED,
        );
    }
    let mut points: Vec<_> = bands
        .iter()
        .map(|&(f, g)| egui::pos2(hz(f), db(g)))
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x));
    if points.len() > 1 {
        painter.add(egui::Shape::line(
            points.clone(),
            egui::Stroke::new(1.5_f32, theme::AMBER),
        ));
    }
    for point in points {
        painter.circle_filled(point, 4.0, theme::AMBER);
    }
    response.on_hover_text("Four stored band settings · log frequency and gain");
}

#[cfg(test)]
mod tests;

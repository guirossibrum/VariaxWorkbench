use super::*;
use line6_devices::{
    backup::LoadedPodxt,
    family::podxt::{BankKind, Location},
};
use pod_preset::{ContainerKind, PodTone};
use variax_workbench::ui::theme;

const TITLE: &str = "Tone Locker";
const INITIAL_SIZE: [f32; 2] = [840.0, 560.0];
const MIN_SIZE: [f32; 2] = [620.0, 340.0];

pub(super) enum Document {
    File(PathBuf, Vec<u8>),
    Backup(Location, PodTone),
    #[cfg(feature = "usb")]
    Device(Location, PodTone),
}

#[derive(Default)]
pub(super) struct LockerWindow {
    open: bool,
    focus: bool,
    toolbar_id: Option<egui::Id>,
    pub(super) files: FileBrowser,
    backup_slot: Option<Location>,
}

impl LockerWindow {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let button = ui.button(TITLE).on_hover_text(
            "Browse local files and PODxt Live banks. CustomTone was a separate online service.",
        );
        self.toolbar_id = Some(button.id);
        if button.clicked() {
            self.open = true;
            self.focus = true;
        }
        button
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        #[cfg(feature = "usb")] live: &mut live_panel::LivePanel,
        backup: Option<&LoadedPodxt>,
        #[cfg(feature = "usb")] editor_tone: Option<&PodTone>,
        document_pending: bool,
    ) -> Option<Document> {
        if !self.open {
            return None;
        }
        let id = egui::Id::new(("podxt-tone-locker", services.tab));
        let focus = std::mem::take(&mut self.focus);
        let enabled = ui.is_enabled() && !document_pending;
        let mut open = true;
        let mut document = None;
        ui.ctx().show_viewport_immediate(
            egui::ViewportId(id),
            egui::ViewportBuilder::default()
                .with_title("Variax Workbench · Tone Locker")
                .with_inner_size(INITIAL_SIZE)
                .with_min_inner_size(MIN_SIZE)
                .with_resizable(true),
            |ctx, class| {
                let embedded = class == egui::ViewportClass::Embedded;
                let mut close = false;
                let content = |ui: &mut egui::Ui| {
                    ui.set_style(theme::style());
                    let close_button = ui
                        .horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.button("Close")
                            })
                            .inner
                        })
                        .inner;
                    if focus {
                        close_button.request_focus();
                    }
                    let escape_focused = if embedded {
                        close_button.has_focus()
                            || ctx
                                .input(|i| i.pointer.hover_pos())
                                .is_some_and(|pos| ctx.layer_id_at(pos) == Some(ui.layer_id()))
                    } else {
                        ctx.input(|i| i.focused)
                    };
                    close = close_button.clicked()
                        || escape_focused
                            && ctx.input_mut(|i| {
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                            });
                    if close {
                        return;
                    }
                    ui.add_enabled_ui(enabled, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("locker-content")
                            .show(ui, |ui| {
                                #[cfg(feature = "usb")]
                                if live.connected() || live.locker_dirty() {
                                    document = live.show_locker(
                                        ui,
                                        services,
                                        &mut self.files,
                                        editor_tone,
                                    );
                                    return;
                                }
                                ui.columns(2, |columns| {
                                    document = self.files.show(
                                        &mut columns[0],
                                        services,
                                        #[cfg(feature = "usb")]
                                        None,
                                    );
                                    if let Some(bytes) = self.saved_library(&mut columns[1], backup)
                                    {
                                        document = Some(bytes);
                                    }
                                });
                            });
                    });
                };
                if embedded {
                    let response = egui::Window::new(TITLE)
                        .id(id)
                        .open(&mut open)
                        .default_size(INITIAL_SIZE)
                        .min_width(MIN_SIZE[0])
                        .min_height(MIN_SIZE[1])
                        .collapsible(false)
                        .show(ctx, content);
                    if focus && let Some(response) = response {
                        ctx.move_to_top(response.response.layer_id);
                    }
                } else if ctx.input(|i| i.viewport().close_requested()) {
                    open = false;
                } else {
                    if focus {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    egui::CentralPanel::default().show(ctx, content);
                }
                open &= !close;
            },
        );
        self.open = open;
        if !open {
            if let Some(id) = self.toolbar_id {
                ui.ctx().memory_mut(|memory| memory.request_focus(id));
            }
            ui.ctx().request_repaint();
        }
        document
    }

    fn saved_library(
        &mut self,
        ui: &mut egui::Ui,
        backup: Option<&LoadedPodxt>,
    ) -> Option<Document> {
        ui.strong("PODxt Live");
        #[cfg(not(feature = "usb"))]
        ui.weak("Device actions are unavailable in this build.");
        #[cfg(feature = "usb")]
        ui.weak("Connect a verified PODxt Live to read its library.");
        read_library_button(ui, false);
        ui.label(if backup.is_some() {
            "Source: imported library backup"
        } else {
            "Use File actions → Use library backup… to browse a saved library."
        });
        let mut document = None;
        egui::ScrollArea::vertical()
            .id_salt("locker-backup")
            .max_height(list_height(ui))
            .show(ui, |ui| {
                for bank in BankKind::ALL {
                    let entries = backup
                        .into_iter()
                        .flat_map(|backup| &backup.entries)
                        .filter(|entry| entry.location.kind() == bank);
                    bank_group(ui, bank, entries.clone().count(), |ui| {
                        for entry in entries {
                            let slot = entry.location;
                            if ui
                                .selectable_label(
                                    self.backup_slot == Some(slot),
                                    format!("{} · {}", slot.slot_name(), entry.tone.name()),
                                )
                                .clicked()
                            {
                                self.backup_slot = Some(slot);
                            }
                        }
                    });
                }
            });
        if let Some(entry) = self
            .backup_slot
            .and_then(|slot| backup?.entries.iter().find(|entry| entry.location == slot))
        {
            ui.label(format!("Backup slot: {}", entry.location.slot_name()));
            if ui.button("Open backup copy in editor").clicked()
                && let Some(tone) = stage::decode_tone(&entry.tone, entry.location.kind())
            {
                document = Some(Document::Backup(entry.location, tone));
            }
        }
        document
    }
}

pub(super) fn list_height(ui: &egui::Ui) -> f32 {
    (ui.ctx().content_rect().height() * 0.4).max(125.0)
}

pub(super) fn read_library_button(ui: &mut egui::Ui, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new("Read full library"))
        .on_hover_text("Get bundle reads all 299 stored tones and setups from PODxt Live.")
        .clicked()
}

pub(super) fn bank_group(
    ui: &mut egui::Ui,
    bank: BankKind,
    loaded: usize,
    entries: impl FnOnce(&mut egui::Ui),
) {
    let state = if loaded == 0 {
        "Not loaded".into()
    } else {
        format!("{loaded}/{} loaded", bank.size())
    };
    let scope = match bank {
        BankKind::Tones => "Complete presets on channels 1A–32D.",
        BankKind::AmpSetups => "Defaults for each amp model, including amp, cab and mic settings.",
        BankKind::FxSetups => "Reusable effect settings and signal routing.",
    };
    egui::CollapsingHeader::new(format!("{} · {state}", bank.name()))
        .id_salt(bank.name())
        .show(ui, |ui| {
            ui.small(scope);
            if loaded > 0 {
                entries(ui);
            }
        });
}

#[derive(Default)]
pub(super) struct FileBrowser {
    directory: Option<PathBuf>,
    files: Vec<PathBuf>,
    file: Option<PathBuf>,
    pub(super) clipboard: Option<PodTone>,
    status: String,
}

#[cfg(feature = "usb")]
#[derive(Clone)]
pub(super) enum Drag {
    Device(Location),
    File(PathBuf),
}

#[cfg(feature = "usb")]
fn write_file_name(slot: Location) -> String {
    format!("{}.l6t", slot.slot_name().replace(' ', "-"))
}

pub(super) fn read_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    pod_preset::read_regular_bundle(pod_preset::path_facts(path), || {
        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        Ok((
            pod_preset::PathFacts {
                regular_file: metadata.is_file(),
                len: metadata.len(),
            },
            file,
        ))
    })
    .map_err(|error| error.detail())
}

impl FileBrowser {
    pub(super) fn use_document_directory(&mut self, path: Option<&std::path::Path>) {
        if self.directory.is_none()
            && let Some(directory) = path.and_then(std::path::Path::parent)
        {
            self.directory = Some(directory.to_path_buf());
            if let Err(error) = self.refresh_files() {
                self.status = error;
            }
        }
    }

    fn refresh_files(&mut self) -> Result<(), String> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        self.files = fs::read_dir(directory)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        ["l6t", "tone", "l6a", "l6s", "l6f", "l6b", "l6c", "vwb"]
                            .iter()
                            .any(|kind| ext.eq_ignore_ascii_case(kind))
                    })
            })
            .collect();
        self.files.sort();
        Ok(())
    }

    fn remember(&mut self, result: Result<(), String>, success: &str) {
        self.status = match result {
            Ok(()) => success.to_owned(),
            Err(error) => error,
        };
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        #[cfg(feature = "usb")] device_tone: Option<&dyn Fn(Location) -> Result<PodTone, String>>,
    ) -> Option<Document> {
        let mut open_document = None;
        ui.strong("Files");
        if ui.button("Choose folder…").clicked() {
            services.request_picker::<PodxtEditor>(
                ui.ctx(),
                PickRequest::Folder,
                |editor, path, _| {
                    if let Some(path) = path {
                        let locker = &mut editor.locker.files;
                        locker.directory = Some(path);
                        locker.file = None;
                        let result = locker.refresh_files();
                        locker.remember(result, "");
                    }
                },
            );
        }
        if let Some(dir) = &self.directory {
            ui.label(dir.display().to_string());
        } else {
            ui.weak("Choose a folder to browse local tones and bundles.");
        }
        let _area = egui::ScrollArea::vertical()
            .id_salt("locker-files")
            .max_height(list_height(ui))
            .show(ui, |ui| {
                for path in &self.files {
                    let response = ui.selectable_label(
                        self.file.as_ref() == Some(path),
                        path.file_name().unwrap_or_default().to_string_lossy(),
                    );
                    let response = response.interact(egui::Sense::click_and_drag());
                    #[cfg(feature = "usb")]
                    if device_tone.is_some() {
                        response.dnd_set_drag_payload(Drag::File(path.clone()));
                    }
                    if response.clicked() {
                        self.file = Some(path.clone());
                    }
                }
                #[cfg(feature = "usb")]
                if self.directory.is_some() && device_tone.is_some() {
                    ui.weak("Drop a PODxt Live entry here to save a new file.");
                }
            });
        #[cfg(feature = "usb")]
        if let Some(device_tone) = device_tone
            && ui
                .interact(
                    _area.inner_rect,
                    ui.id().with("locker-file-drop"),
                    egui::Sense::hover(),
                )
                .dnd_release_payload::<Drag>()
                .is_some_and(|drag| {
                    let Drag::Device(slot) = drag.as_ref() else {
                        return false;
                    };
                    let result = (|| {
                        let dir = self.directory.as_ref().ok_or("Choose a folder first.")?;
                        let destination = dir.join(write_file_name(*slot));
                        device_tone(*slot)?
                            .save_copy(&destination)
                            .map_err(|error| error.detail())?;
                        self.refresh_files()?;
                        Ok(format!(
                            "Saved {} → {}",
                            slot.slot_name(),
                            destination.display()
                        ))
                    })();
                    self.status = result.unwrap_or_else(|error| error);
                    true
                })
        {
            ui.ctx().request_repaint();
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(path) = &self.file {
                let open = ui.button("Open in editor").clicked();
                let copy = ui.button("Copy file tone").clicked();
                if open {
                    match read_file(path) {
                        Ok(bytes) => open_document = Some(Document::File(path.clone(), bytes)),
                        Err(error) => self.status = error,
                    }
                }
                if copy {
                    match read_file(path)
                        .and_then(|bytes| PodTone::parse(bytes).map_err(|error| error.detail()))
                    {
                        Ok(tone) => self.clipboard = Some(tone),
                        Err(error) => self.status = error,
                    }
                }
            }
            if let Some(tone) = &self.clipboard
                && ui.button("Save clipboard as new file…").clicked()
            {
                let name = match tone.container() {
                    ContainerKind::L6pa => "Tone copy.l6t",
                    ContainerKind::L6as => "Amp Setup copy.l6t",
                    ContainerKind::L6fs => "FX Setup copy.l6t",
                };
                let tone = tone.clone();
                services.request_picker::<PodxtEditor>(
                    ui.ctx(),
                    PickRequest::SaveTone { name: name.into() },
                    move |editor, path, _| {
                        if let Some(path) = path {
                            let locker = &mut editor.locker.files;
                            let result = tone
                                .save_copy(&path)
                                .map_err(|error| error.detail())
                                .and_then(|()| locker.refresh_files());
                            locker.remember(result, "Saved tone copy.");
                        }
                    },
                );
            }
        });
        if let Some(tone) = &self.clipboard {
            ui.small(format!("Clipboard: {}", tone.name()));
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        open_document
    }
}

#[cfg(test)]
pub(super) mod tests;

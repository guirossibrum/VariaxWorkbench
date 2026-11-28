use super::{
    EditorDocument, EditorServices, FAMILY, PathBuf, PickRequest, PresetFile, ProductFamily,
    VariaxEditor, egui, tabs,
};
use variax_workbench::document::save_new;

pub(super) struct SlotReplacement {
    selection: (usize, usize),
    previous_name: String,
    model: PresetFile,
}

impl VariaxEditor {
    pub(super) fn slot_file_actions(
        &mut self,
        services: &mut EditorServices<'_>,
        ui: &mut egui::Ui,
        available: bool,
    ) {
        let Some(document) = &self.document else {
            return;
        };
        let library = self.library_label();
        let selection = document.selection();
        if self.parked_library.is_some() || selection.is_none() {
            // Clear the selector reference when there is no active bank.
            self.factory_selector_reference(ui);
            if let Some(library) = library {
                ui.label(library);
            }
            if self.parked_library.is_some() {
                self.file_mode_actions(ui, available);
            } else {
                self.html_export_button(ui, services, available);
            }
            self.device_edit_controls(ui);
            return;
        }
        let enabled = available && document.selected_slot_model().is_ok();
        ui.scope(|ui| {
            let spacing = ui.spacing_mut();
            spacing.interact_size.y = 22.0;
            spacing.item_spacing = egui::vec2(6.0, 2.0);
            spacing.button_padding = egui::vec2(6.0, 2.0);
            ui.horizontal_wrapped(|ui| {
                let model = ui
                    .add_enabled_ui(enabled, |ui| {
                        ui.menu_button("Model actions", |ui| {
                            let save = ui.button("Save model as…");
                            let replace = ui.button("Replace slot with model file…");
                            #[cfg(test)]
                            {
                                self.slot_file_buttons.insert("save-model", save.rect);
                                self.slot_file_buttons.insert("replace-slot", replace.rect);
                            }
                            if save.clicked() {
                                self.save_slot_model(ui.ctx(), services);
                                ui.close();
                            }
                            if replace.clicked() {
                                self.choose_slot_replacement(ui.ctx(), services);
                                ui.close();
                            }
                            #[cfg(feature = "usb")]
                            if let Some(position) =
                                selection.filter(|_| self.has_current_guitar_library())
                            {
                                use super::device_sync::Action;

                                let action = Action::ReadUnmodified;
                                let revert = ui.add_enabled(
                                    self.sync_ready(action),
                                    egui::Button::new("Revert to unmodified model…"),
                                );
                                #[cfg(test)]
                                self.slot_file_buttons.insert("revert-model", revert.rect);
                                if revert.clicked() {
                                    self.confirm_position_action(action, position);
                                    ui.close();
                                }
                            }
                            ui.separator();
                            self.model_transfer_actions(ui);
                        })
                    })
                    .inner;
                let library_menu = ui.menu_button("Library actions", |ui| {
                    if let Some(library) = library {
                        ui.label(library);
                        ui.separator();
                    }
                    let rename = self.rename_bank_button(ui, available);
                    let export = self.html_export_button(ui, services, available);
                    if rename || export {
                        ui.close();
                    }
                    ui.separator();
                    ui.add_enabled_ui(enabled, |ui| self.bank_transfer_actions(ui));
                });
                #[cfg(test)]
                {
                    self.slot_file_buttons
                        .insert("slot-transfers", model.response.rect);
                    self.slot_file_buttons
                        .insert("library-actions", library_menu.response.rect);
                }
                #[cfg(not(test))]
                let _ = (model, library_menu);
                self.factory_selector_reference(ui);
                self.device_edit_controls(ui);
            });
        });
    }

    fn file_mode_actions(&mut self, ui: &mut egui::Ui, available: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(available, |ui| self.library_position(ui));
            let enabled = self.parked_library.as_ref().is_some_and(|library| {
                self.document
                    .as_ref()
                    .and_then(|file| file.selected_preset())
                    .is_some_and(|model| library.document.validate_slot_replacement(model).is_ok())
            });
            let place = ui
                .add_enabled(
                    available && enabled && !self.dirty(),
                    egui::Button::new("Put this file into the selected position"),
                )
                .on_disabled_hover_text(
                    "Save the model first and select a compatible library slot.",
                );
            let back = ui.add_enabled(available, egui::Button::new("Return to library"));
            #[cfg(test)]
            {
                self.slot_file_buttons.insert("place-file", place.rect);
                self.slot_file_buttons.insert("return-library", back.rect);
            }
            if place.clicked() {
                self.place_open_file();
            }
            if back.clicked() {
                if self.dirty() {
                    self.pending =
                        Some(super::GuitarPending::Open(super::OpenDestination::Library));
                } else {
                    self.return_to_library();
                }
            }
        });
    }

    fn library_position(&mut self, ui: &mut egui::Ui) {
        let Some(library) = &mut self.parked_library else {
            return;
        };
        let library = &mut library.document;
        let mut selection = library.selection();
        let label = selection.and_then(|(bank, slot)| {
            Some(format!(
                "{} · Slot {}",
                library.banks()?.get(bank)?.name,
                slot + 1
            ))
        });
        egui::ComboBox::from_id_salt("put-file-position")
            .selected_text(label.unwrap_or_else(|| "No library positions".into()))
            .height(250.0)
            .show_ui(ui, |ui| {
                for (bank, entry) in library.banks().into_iter().flatten().enumerate() {
                    for (slot, preset) in entry.presets().iter().enumerate() {
                        let position = Some((bank, slot));
                        let label = format!("{} · {} {}", entry.name, slot + 1, preset.name());
                        if ui.selectable_label(selection == position, label).clicked() {
                            selection = position;
                            ui.close();
                        }
                    }
                }
            });
        if let Some((bank, slot)) = selection
            && selection != library.selection()
            && let Err(error) = library.select(bank, slot)
        {
            self.error = Some(error);
        }
    }

    fn place_open_file(&mut self) {
        let Some(model) = self
            .document
            .as_ref()
            .and_then(EditorDocument::selected_preset)
        else {
            return;
        };
        let Some(library) = &mut self.parked_library else {
            return;
        };
        match library.document.replace_slot_model(model.clone()) {
            Ok(()) => {
                self.return_to_library();
                self.status = "Model placed in the library. Not sent to guitar.".into();
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn save_slot_model(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        let Some(model) = self
            .document
            .as_ref()
            .and_then(|document| document.selected_slot_model().ok())
        else {
            return;
        };
        let stem = super::model_filename_stem(model.name());
        let stem = if stem.trim().is_empty() {
            "Variax model"
        } else {
            stem.trim()
        };
        let request = PickRequest::SaveWorkbenchFile {
            name: format!("{stem}.vwm"),
            extension: "vwm".into(),
        };
        let bytes = model.as_bytes().to_vec();
        services.request_picker::<Self>(ctx, request, move |editor, path, _| {
            let Some(path) = path else {
                return;
            };
            let path = with_file_extension(path, "vwm");
            match save_new(&path, &bytes) {
                Ok(()) => {
                    editor.error = None;
                    editor.status = format!("Saved model: {}", path.display());
                }
                Err(error) => editor.error = Some(error),
            }
        });
    }

    fn choose_slot_replacement(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        let Some(selection) = self
            .document
            .as_ref()
            .and_then(|document| document.selection())
        else {
            return;
        };
        services.request_picker::<Self>(
            ctx,
            PickRequest::OpenWorkbenchFile,
            move |editor, path, _| {
                if let Some(path) = path {
                    editor.stage_slot_replacement(selection, path);
                }
            },
        );
    }

    fn stage_slot_replacement(&mut self, selection: (usize, usize), path: PathBuf) {
        let Some(document) = self
            .document
            .as_ref()
            .filter(|document| document.selection() == Some(selection))
        else {
            self.error = Some("The selected slot changed. Choose the replacement again.".into());
            return;
        };
        let Ok(previous) = document.selected_slot_model() else {
            return;
        };
        let result = tabs::open_candidate(&path, FAMILY.info().ceiling)
            .and_then(PresetFile::parse)
            .and_then(|model| {
                document.validate_slot_replacement(&model)?;
                Ok(model)
            });
        match result {
            Ok(model) => {
                self.error = None;
                self.slot_replacement = Some(SlotReplacement {
                    selection,
                    previous_name: previous.name().to_owned(),
                    model,
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn slot_replacement_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        #[cfg(not(test))]
        let _ = services;
        let Some(replacement) = self.slot_replacement.take() else {
            return;
        };
        let (_, slot) = replacement.selection;
        let prompt = egui::Modal::new(egui::Id::new("variax-replace-slot")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading("Replace model slot?");
            ui.label(format!(
                "Replace “{}” in slot {} with “{}”?",
                replacement.previous_name,
                slot + 1,
                replacement.model.name(),
            ));
            ui.horizontal(|ui| {
                let cancel = ui.button("Cancel");
                let confirm = ui.button("Replace slot");
                #[cfg(test)]
                {
                    services.prompt_buttons.insert("cancel-slot", cancel.rect);
                    services.prompt_buttons.insert("confirm-slot", confirm.rect);
                }
                (cancel.clicked(), confirm.clicked())
            })
            .inner
        });
        let (cancel, confirm) = prompt.inner;
        if cancel || prompt.should_close() {
            return;
        }
        if !confirm {
            self.slot_replacement = Some(replacement);
            return;
        }

        let Some(document) = self
            .document
            .as_mut()
            .filter(|document| document.selection() == Some(replacement.selection))
        else {
            self.error = Some("The selected slot changed. Choose the replacement again.".into());
            return;
        };
        match document.replace_slot_model(replacement.model) {
            Ok(()) => {
                self.status = format!(
                    "Slot {} now contains “{}”. Not sent to guitar.",
                    slot + 1,
                    document.preset().name()
                );
                self.error = None;
                self.drag_origin = None;
                self.clear_text_drafts();
            }
            Err(error) => self.error = Some(error),
        }
    }
}

pub(super) fn with_file_extension(path: PathBuf, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|current| current.eq_ignore_ascii_case(extension))
    {
        path
    } else {
        let mut name = path.into_os_string();
        name.push(".");
        name.push(extension);
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tests::{app, click_prompt};
    use crate::editor::{FakeDialogs, Workbench};
    use std::fs;
    use variax_workbench::{bundle::BundleFile, preset::PresetText};

    pub(super) fn click_slot_action(ctx: &egui::Context, app: &mut Workbench, id: &'static str) {
        fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1120.0, 760.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
                    app.pending_dialog(ctx);
                    app.poll_picker(ctx);
                },
            );
        }

        let click = |app: &mut Workbench, pos| {
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
            frame(ctx, app, vec![button(false)]);
            frame(ctx, app, vec![]);
        };
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        if matches!(id, "save-model" | "replace-slot" | "revert-model") {
            click(
                app,
                app.variax().slot_file_buttons["slot-transfers"].center(),
            );
        }
        click(app, app.variax().slot_file_buttons[id].center());
    }

    #[cfg(all(feature = "usb", target_os = "linux"))]
    #[test]
    fn model_actions_opens_the_existing_revert_confirmation() {
        use super::super::refresh::tests::{attach, download};

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        attach(editor);
        download(editor, &ctx);
        let document = editor.document.as_mut().unwrap();
        document.select(1, 2).unwrap();
        let saved = document.as_bytes().to_vec();
        editor.change(|model| model.rename("Local draft"));
        let edited = editor.document.as_ref().unwrap().as_bytes().to_vec();

        click_slot_action(&ctx, &mut app, "revert-model");
        assert!(app.variax().device_sync.prompting());
        assert!(!app.variax().device_sync.busy());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), edited);

        crate::editor::tests::prompt_frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let editor = app.variax_mut();
        assert!(!editor.device_sync.prompting());
        assert!(!editor.device_sync.busy());
        let document = editor.document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((1, 2)));
        assert_eq!(document.as_bytes(), edited);
        editor.undo();
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), saved);
        editor.redo();
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), edited);
    }

    #[test]
    fn save_model_exports_only_the_selected_slot_and_keeps_the_bank_dirty() {
        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.vwb");
        let bank = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        save_new(&source, &bank).unwrap();
        app.variax_mut().load(&source);
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .select(0, 1)
            .unwrap();
        app.variax_mut()
            .change(|model| model.rename("Current slot"));
        let document = app.variax().document.as_ref().unwrap();
        let edited = document.as_bytes().to_vec();
        let model = document.preset().as_bytes().to_vec();

        let existing = dir.path().join("existing.VWM");
        let destination = dir.path().join("exported");
        save_new(&existing, b"keep existing bytes").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            save: [None, Some(existing.clone()), Some(destination.clone())].into(),
            ..Default::default()
        });
        click_slot_action(&ctx, &mut app, "save-model");
        assert!(app.variax().error.is_none());
        click_slot_action(&ctx, &mut app, "save-model");
        assert!(app.variax().error.is_some());
        assert_eq!(fs::read(&existing).unwrap(), b"keep existing bytes");

        click_slot_action(&ctx, &mut app, "save-model");
        assert!(app.variax().error.is_none());
        let exported = destination.with_extension("vwm");
        assert_eq!(fs::read(&exported).unwrap(), model);
        assert_eq!(
            EditorDocument::open(&exported).unwrap().preset().name(),
            "Current slot"
        );
        let document = app.variax().document.as_ref().unwrap();
        assert!(document.is_dirty());
        assert_eq!(document.selection(), Some((0, 1)));
        assert_eq!(document.as_bytes(), edited);
        assert_eq!(fs::read(&source).unwrap(), bank);
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), bank);
    }

    #[test]
    fn replace_slot_rejects_bad_files_and_requires_confirmation_before_one_undoable_edit() {
        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.vwb");
        let bank = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        save_new(&source, &bank).unwrap();
        app.variax_mut().load(&source);
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .select(0, 1)
            .unwrap();
        app.variax_mut()
            .change(|model| model.rename("Keep earlier edit"));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

        let fixture = include_bytes!("../../../../tests/fixtures/editor-preset.vwm");
        let pod = dir.path().join("pod.vwm");
        let incompatible = dir.path().join("acoustic.vwm");
        let mut bytes = fixture.to_vec();
        for (path, id) in [(&pod, 0x0003_000a_u32), (&incompatible, 0x0007_0001)] {
            bytes[24..28].copy_from_slice(&id.to_be_bytes());
            save_new(path, &bytes).unwrap();
        }
        let malformed = dir.path().join("malformed.vwm");
        save_new(&malformed, b"FORM").unwrap();

        bytes[24..28].copy_from_slice(&0x0007_0000_u32.to_be_bytes());
        let mut replacement = PresetFile::parse(bytes).unwrap();
        replacement.rename("Replacement").unwrap();
        replacement
            .set_metadata(PresetText::Notes, "Resized, with notes")
            .unwrap();
        let model_file = dir.path().join("replacement.vwm");
        save_new(&model_file, replacement.as_bytes()).unwrap();
        let bank_copy = dir.path().join("bank-copy.vwb");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(bank_copy.clone())].into(),
            open: [
                None,
                Some(malformed),
                Some(pod),
                Some(incompatible),
                Some(source.clone()),
                Some(model_file.clone()),
                Some(model_file.clone()),
                Some(bank_copy.clone()),
            ]
            .into(),
            ..Default::default()
        });

        click_slot_action(&ctx, &mut app, "replace-slot");
        assert!(!app.has_prompt());
        for error in [
            "individual FORM",
            "PODxt Live",
            "300/600 or 500/700",
            "individual FORM",
        ] {
            click_slot_action(&ctx, &mut app, "replace-slot");
            assert!(!app.has_prompt());
            assert!(app.variax().error.as_deref().unwrap().contains(error));
            assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        }

        click_slot_action(&ctx, &mut app, "replace-slot");
        assert!(app.has_prompt());
        click_prompt(&ctx, &mut app, "cancel-slot");
        assert!(!app.has_prompt());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

        click_slot_action(&ctx, &mut app, "replace-slot");
        assert!(app.has_prompt());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        click_prompt(&ctx, &mut app, "confirm-slot");
        assert!(!app.has_prompt());
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((0, 1)));
        assert_eq!(document.preset().as_bytes(), replacement.as_bytes());
        assert!(document.slot_is_dirty(0, 1));
        let changed = document.as_bytes().to_vec();
        let old_bank = BundleFile::parse(before.clone()).unwrap();
        let new_bank = BundleFile::parse(changed.clone()).unwrap();
        for (bank, (old, new)) in old_bank.banks().iter().zip(new_bank.banks()).enumerate() {
            assert_eq!(old.name, new.name);
            for (slot, (old, new)) in old.presets().iter().zip(new.presets()).enumerate() {
                if (bank, slot) != (0, 1) {
                    assert_eq!(old.as_bytes(), new.as_bytes());
                }
            }
        }
        assert!(FAMILY.claims(&changed));
        assert!(FAMILY.claims(replacement.as_bytes()));

        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        app.variax_mut().redo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), changed);
        assert_eq!(fs::read(source).unwrap(), bank);
        assert_eq!(fs::read(model_file).unwrap(), replacement.as_bytes());

        app.save_dialog();
        assert_eq!(fs::read(&bank_copy).unwrap(), changed);
        let tab_count = app.tabs.entries.len();
        app.open_file_dialog();
        assert!(app.shell_error.is_none());
        assert_eq!(app.tabs.entries.len(), tab_count + 1);
        let active = app.tabs.index(app.tabs.active.unwrap()).unwrap();
        assert_eq!(app.tabs.entries[active].family.info().id, "variax");
    }
}

#[cfg(test)]
#[test]
fn opening_then_placing_a_file_uses_the_existing_position_and_one_undo_step() {
    use crate::editor::tests::app;
    let (ctx, mut app) = app();
    app.variax_mut().open_example();
    let folder = tempfile::tempdir().unwrap();
    let library = folder.path().join("library.vwb");
    let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    save_new(&library, &original).unwrap();
    app.variax_mut().load(&library);
    app.variax_mut()
        .document
        .as_mut()
        .unwrap()
        .select(1, 2)
        .unwrap();
    let mut model = app.variax().document.as_ref().unwrap().preset().clone();
    model.rename("File mode model").unwrap();
    let file = folder.path().join("Model from a file.vwm");
    save_new(&file, model.as_bytes()).unwrap();
    app.dialogs = Box::new(crate::editor::FakeDialogs {
        open: [Some(file.clone())].into(),
        ..Default::default()
    });

    let old_tab = app.tabs.active;
    app.open_dialog();
    assert_eq!(app.tabs.active, old_tab);
    let editor = app.variax();
    assert_eq!(editor.source_path.as_deref(), Some(file.as_path()));
    assert_eq!(editor.document.as_ref().unwrap().selection(), None);
    let parked = &editor.parked_library.as_ref().unwrap().document;
    assert_eq!(parked.as_bytes(), original);
    assert_eq!(parked.selection(), Some((1, 2)));

    tests::click_slot_action(&ctx, &mut app, "place-file");
    let editor = app.variax();
    let document = editor.document.as_ref().unwrap();
    assert_eq!(editor.source_path.as_deref(), Some(library.as_path()));
    assert_eq!(document.selection(), Some((1, 2)));
    assert_eq!(document.preset().as_bytes(), model.as_bytes());
    assert!(document.slot_is_unsent(1, 2));
    assert!(editor.status.contains("Not sent to guitar"));
    assert_eq!(std::fs::read(&library).unwrap(), original);
    assert_eq!(std::fs::read(&file).unwrap(), model.as_bytes());
    app.variax_mut().undo();
    let document = app.variax().document.as_ref().unwrap();
    assert_eq!(document.as_bytes(), original);
    assert!(!document.slot_is_unsent(1, 2));
    app.variax_mut().redo();
    assert!(app.variax().document.as_ref().unwrap().slot_is_unsent(1, 2));
}

use super::{EditorServices, FAMILY, PathBuf, PresetFile, ProductFamily, VariaxEditor, egui, tabs};
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
        if document.selection().is_none() {
            self.html_export_button(ui, services, available);
            return;
        }
        let enabled = available && document.selected_slot_model().is_ok();
        ui.horizontal_wrapped(|ui| {
            let save = ui.add_enabled(enabled, egui::Button::new("Save model as…").small());
            let replace = ui.add_enabled(
                enabled,
                egui::Button::new("Replace slot with model file…").small(),
            );
            #[cfg(test)]
            {
                self.slot_file_buttons.insert("save-model", save.rect);
                self.slot_file_buttons.insert("replace-slot", replace.rect);
            }
            if save.clicked() {
                self.save_slot_model(services);
            }
            if replace.clicked() {
                self.choose_slot_replacement(services);
            }
            self.slot_transfer_menu(ui, enabled);
            self.rename_bank_button(ui, available);
            self.html_export_button(ui, services, available);
        });
    }

    fn save_slot_model(&mut self, services: &mut EditorServices<'_>) {
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
        let Some(path) = services.dialogs.save(&format!("{stem}.vwm"), "vwm") else {
            return;
        };
        let path = with_file_extension(path, "vwm");
        match save_new(&path, model.as_bytes()) {
            Ok(()) => {
                self.error = None;
                self.status = format!("Saved model: {}", path.display());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn choose_slot_replacement(&mut self, services: &mut EditorServices<'_>) {
        let Some(document) = &self.document else {
            return;
        };
        let (Some(selection), Ok(previous)) =
            (document.selection(), document.selected_slot_model())
        else {
            return;
        };
        let Some(path) = services.dialogs.open() else {
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
                    "Slot {} now contains “{}”.",
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
    use variax_workbench::{bundle::BundleFile, document::EditorDocument, preset::PresetText};

    fn click_slot_action(ctx: &egui::Context, app: &mut Workbench, id: &'static str) {
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
                },
            );
        }

        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let pos = app.variax().slot_file_buttons[id].center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
        frame(ctx, app, vec![button(false)]);
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

        assert!(app.save_dialog());
        assert_eq!(fs::read(&bank_copy).unwrap(), changed);
        let tab_count = app.tabs.entries.len();
        app.open_file_dialog();
        assert!(app.shell_error.is_none());
        assert_eq!(app.tabs.entries.len(), tab_count + 1);
        let active = app.tabs.index(app.tabs.active.unwrap()).unwrap();
        assert_eq!(app.tabs.entries[active].family.info().id, "variax");
    }
}

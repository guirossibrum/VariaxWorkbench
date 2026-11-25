use super::{
    EditorDocument, EditorServices, Path, PathBuf, PickRequest, PresetFile, VariaxEditor, egui,
};
#[cfg(feature = "usb")]
use variax_workbench::bundle::BackupBundle;
use variax_workbench::{bundle::BundleFile, document::save_new};

#[cfg(all(test, feature = "usb"))]
#[path = "../file/bundle/synthetic_backup.rs"]
pub(super) mod synthetic_backup;

#[derive(Clone, Copy)]
enum FileKind {
    Model,
    Bundle,
    Collection,
}

impl FileKind {
    fn commands(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Model => ("New Model…", "Default Patch.vwm", "vwm"),
            Self::Bundle => ("New Bundle…", "New Bundle.vwb", "vwb"),
            Self::Collection => ("New Collection…", "New Collection.l6c", "l6c"),
        }
    }

    fn create(self, path: PathBuf, author: &str) -> Result<EditorDocument, String> {
        match self {
            Self::Model => Ok(EditorDocument::from_preset(
                PresetFile::default_legacy_model_with_author(author)?,
                path,
            )),
            Self::Bundle => EditorDocument::from_bundle(
                BundleFile::new_default_bundle_with_author(author)?,
                path,
            ),
            Self::Collection => EditorDocument::from_bundle(BundleFile::new_collection()?, path),
        }
    }
}

pub(super) struct NewFile {
    path: PathBuf,
    contents: NewFileContents,
}

enum NewFileContents {
    Template(FileKind),
    #[cfg(feature = "usb")]
    Backup(BackupBundle),
}

impl NewFile {
    pub(super) fn destination(&self, path: &Path) -> Result<PathBuf, String> {
        match &self.contents {
            NewFileContents::Template(_) => Ok(path.to_owned()),
            #[cfg(feature = "usb")]
            NewFileContents::Backup(backup) => backup.destination(path),
        }
    }
}

pub(super) struct BankRename {
    bank: usize,
    name: String,
    error: Option<String>,
    focus: bool,
}

impl VariaxEditor {
    pub(super) fn new_file_menu(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        let menu = ui.menu_button("New Variax file", |ui| {
            for kind in [FileKind::Model, FileKind::Bundle, FileKind::Collection] {
                let (label, suggested, extension) = kind.commands();
                let response = ui.button(label);
                #[cfg(test)]
                self.new_file_buttons.insert(label, response.rect);
                if response.clicked() {
                    ui.close();
                    let request = PickRequest::SaveWorkbenchFile {
                        name: suggested.into(),
                        extension: extension.into(),
                    };
                    services.request_picker::<Self>(ui.ctx(), request, move |editor, path, _| {
                        if let Some(path) = path {
                            editor.queue_new_file(NewFile {
                                path: super::slot_files::with_file_extension(path, extension),
                                contents: NewFileContents::Template(kind),
                            });
                        }
                    });
                }
            }
            #[cfg(feature = "usb")]
            {
                ui.separator();
                let label = "Open backup folder as .vwb…";
                let response = ui.button(label);
                #[cfg(test)]
                self.new_file_buttons.insert(label, response.rect);
                if response.clicked() {
                    ui.close();
                    self.choose_backup_file(ui.ctx(), services);
                }
            }
        });
        #[cfg(test)]
        self.new_file_buttons
            .insert("New Variax file", menu.response.rect);
        #[cfg(not(test))]
        let _ = menu;
    }

    #[cfg(feature = "usb")]
    fn choose_backup_file(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        let completion_ctx = ctx.clone();
        services.request_picker::<Self>(
            ctx,
            PickRequest::Folder,
            move |editor, directory, services| {
                let Some(directory) = directory else {
                    return;
                };
                let backup = match BackupBundle::load(&directory) {
                    Ok(backup) => backup,
                    Err(error) => {
                        editor.error = Some(error);
                        return;
                    }
                };
                let request = PickRequest::SaveWorkbenchFile {
                    name: "Variax backup.vwb".into(),
                    extension: "vwb".into(),
                };
                services.request_picker::<Self>(
                    &completion_ctx,
                    request,
                    move |editor, path, _| {
                        if let Some(path) = path {
                            editor.queue_new_file(NewFile {
                                path: super::slot_files::with_file_extension(path, "vwb"),
                                contents: NewFileContents::Backup(backup),
                            });
                        }
                    },
                );
            },
        );
    }

    fn queue_new_file(&mut self, pending: NewFile) {
        if let Err(error) = pending.destination(&pending.path) {
            self.error = Some(error);
            return;
        }
        self.new_file = Some(pending);
        if !self.dirty() {
            self.create_pending_file();
        }
    }

    pub(super) fn create_pending_file(&mut self) {
        let Some(pending) = self.new_file.take() else {
            return;
        };
        let result = (|| -> Result<_, String> {
            let path = pending.destination(&pending.path)?;
            let (document, status) = match pending.contents {
                NewFileContents::Template(kind) => (
                    kind.create(path.clone(), &self.new_model_author())?,
                    format!("Created {}", path.display()),
                ),
                #[cfg(feature = "usb")]
                NewFileContents::Backup(backup) => {
                    let captured = backup.captured_bank_names();
                    let document = EditorDocument::from_bundle(backup.into_bundle(), path.clone())?;
                    let status = format!(
                        "Opened {} · {captured}/12 captured bank names. Tone positions may reflect the live knob.",
                        path.display(),
                    );
                    (document, status)
                }
            };
            save_new(&path, document.as_bytes())?;
            Ok((document, path, status))
        })();
        match result {
            Ok((document, path, status)) => {
                self.adopt_document(document, Some(&path));
                self.status = status;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn rename_bank_button(&mut self, ui: &mut egui::Ui, available: bool) {
        let Some((bank, current)) = self
            .document
            .as_ref()
            .and_then(EditorDocument::selected_bank)
        else {
            return;
        };
        let label = if current.fixed {
            "Rename bank…"
        } else {
            "Rename folder…"
        };
        let response = ui.add_enabled(available, egui::Button::new(label).small());
        #[cfg(test)]
        self.new_file_buttons.insert(label, response.rect);
        if response.clicked() {
            self.bank_rename = Some(BankRename {
                bank,
                name: current.name.clone(),
                error: None,
                focus: true,
            });
        }
    }

    pub(super) fn rename_bank_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        #[cfg(not(test))]
        let _ = services;
        let Some(mut draft) = self.bank_rename.take() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("variax-rename-bank")).show(ctx, |ui| {
            ui.heading("Rename bank or folder");
            ui.label("Use up to 18 printable ASCII characters.");
            let response = ui.text_edit_singleline(&mut draft.name);
            if draft.focus {
                response.request_focus();
                draft.focus = false;
            }
            #[cfg(test)]
            self.new_file_buttons.insert("bank name", response.rect);
            let enter =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if let Some(error) = &draft.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            ui.horizontal(|ui| {
                let cancel = ui.button("Cancel");
                let rename = ui.button("Rename");
                #[cfg(test)]
                {
                    services
                        .prompt_buttons
                        .insert("cancel-bank-rename", cancel.rect);
                    services
                        .prompt_buttons
                        .insert("confirm-bank-rename", rename.rect);
                }
                (cancel.clicked(), rename.clicked() || enter)
            })
            .inner
        });
        let (cancel, rename) = modal.inner;
        if cancel || modal.should_close() {
            return;
        }
        if rename {
            let Some(document) = self.document.as_mut().filter(|document| {
                document
                    .selected_bank()
                    .is_some_and(|(bank, _)| bank == draft.bank)
            }) else {
                self.error = Some("The selected bank or folder changed.".into());
                return;
            };
            match document.rename_selected_bank(&draft.name) {
                Ok(()) => {
                    self.status = "Updated the bank or folder name.".into();
                    self.error = None;
                    return;
                }
                Err(error) => draft.error = Some(error),
            }
        }
        self.bank_rename = Some(draft);
    }

    pub(super) fn empty_collection(
        &mut self,
        ui: &mut egui::Ui,
        available: bool,
        services: &mut EditorServices<'_>,
    ) {
        let Some((_, folder)) = self
            .document
            .as_ref()
            .and_then(EditorDocument::selected_bank)
        else {
            return;
        };
        ui.heading(if folder.name.is_empty() {
            "Collection folder"
        } else {
            folder.name.as_str()
        });
        ui.label("This collection folder has no models.");
        ui.horizontal_wrapped(|ui| {
            self.rename_bank_button(ui, available);
            self.html_export_button(ui, services, available);
            let response = ui.add_enabled(available, egui::Button::new("Add default model"));
            #[cfg(test)]
            self.new_file_buttons
                .insert("Add default model", response.rect);
            if response.clicked() {
                self.append_default_model();
            }
        });
    }

    fn append_default_model(&mut self) {
        let author = self.new_model_author();
        let Some(document) = &mut self.document else {
            return;
        };
        match PresetFile::default_legacy_model_with_author(&author)
            .and_then(|model| document.append_collection_model(model))
        {
            Ok(()) => {
                self.error = None;
                self.status = "Added the default model to the collection.".into();
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tests::{app, click_prompt};
    use crate::editor::{FakeDialogs, Workbench};
    use std::fs;

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
            |ctx| app.render(ctx),
        );
    }

    fn click(ctx: &egui::Context, app: &mut Workbench, label: &'static str) {
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let pos = app.variax().new_file_buttons[label].center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
        frame(ctx, app, vec![button(false)]);
    }

    fn choose_file(ctx: &egui::Context, app: &mut Workbench, label: &'static str) {
        click(ctx, app, "New Variax file");
        click(ctx, app, label);
        frame(ctx, app, vec![]);
    }

    #[test]
    fn file_creation_uses_the_remembered_author_for_new_models() {
        use variax_workbench::preset::PresetText;

        let (ctx, mut app) = app();
        let root = tempfile::tempdir().unwrap();
        let artwork = root.path().join("artwork");
        fs::write(
            root.path().join("variax-preferences.json"),
            br#"{"default_author":"Ana"}"#,
        )
        .unwrap();
        app.variax_mut().preferences_path = Some(root.path().join("editor-preferences.txt"));
        let model = root.path().join("model.vwm");
        let bundle = root.path().join("models.vwb");
        let collection = root.path().join("empty.l6c");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(model.clone()), Some(bundle.clone()), Some(collection)].into(),
            artwork: [Some(artwork)].into(),
            ..Default::default()
        });
        app.artwork_dialog();

        choose_file(&ctx, &mut app, "New Model…");
        let document = EditorDocument::open(&model).unwrap();
        assert_eq!(
            document.preset().metadata(PresetText::Author).as_deref(),
            Ok("Ana")
        );

        choose_file(&ctx, &mut app, "New Bundle…");
        let document = EditorDocument::open(&bundle).unwrap();
        let models = document
            .banks()
            .unwrap()
            .iter()
            .flat_map(|bank| bank.presets());
        assert_eq!(
            models
                .filter(|preset| preset.metadata(PresetText::Author).as_deref() == Ok("Ana"))
                .count(),
            60
        );

        choose_file(&ctx, &mut app, "New Collection…");
        app.variax_mut().append_default_model();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(
            document.preset().metadata(PresetText::Author).as_deref(),
            Ok("Ana")
        );
        assert!(document.is_dirty() && document.can_undo());
    }

    #[test]
    fn file_menu_creates_models_bundles_and_collections_without_overwriting_or_losing_edits() {
        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing.vwm");
        save_new(&collision, b"original").unwrap();
        let model = directory.path().join("model");
        let bundle = directory.path().join("new.vwb");
        let collection = directory.path().join("empty.l6c");
        let populated = directory.path().join("populated.l6c");
        app.dialogs = Box::new(FakeDialogs {
            save: [
                Some(collision.clone()),
                Some(model.clone()),
                Some(bundle.clone()),
                Some(bundle.clone()),
                Some(collection.clone()),
                Some(populated.clone()),
            ]
            .into(),
            open: [Some(collection.clone())].into(),
            ..Default::default()
        });

        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        choose_file(&ctx, &mut app, "New Model…");
        assert_eq!(fs::read(&collision).unwrap(), b"original");
        assert!(app.variax().error.is_some());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

        choose_file(&ctx, &mut app, "New Model…");
        let model = model.with_extension("vwm");
        let defaults = PresetFile::default_legacy_model().unwrap();
        assert_eq!(
            app.variax().document.as_ref().unwrap().as_bytes(),
            defaults.as_bytes()
        );
        assert_eq!(fs::read(model).unwrap(), defaults.as_bytes());

        app.variax_mut()
            .change(|preset| preset.rename("Keep this edit"));
        let edited = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        choose_file(&ctx, &mut app, "New Bundle…");
        assert!(app.has_prompt());
        assert!(!bundle.exists());
        click_prompt(&ctx, &mut app, "cancel");
        assert!(!bundle.exists());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), edited);

        choose_file(&ctx, &mut app, "New Bundle…");
        click_prompt(&ctx, &mut app, "discard");
        let doc = app.variax().document.as_ref().unwrap();
        assert_eq!(doc.banks().unwrap().len(), 12);
        assert_eq!(doc.as_bytes(), fs::read(&bundle).unwrap());
        assert!(!doc.is_dirty());

        choose_file(&ctx, &mut app, "New Collection…");
        let doc = app.variax().document.as_ref().unwrap();
        assert!(doc.selected_preset().is_none());
        assert_eq!(doc.as_bytes(), fs::read(&collection).unwrap());
        click(&ctx, &mut app, "Add default model");
        assert!(app.variax().dirty());
        assert_eq!(
            app.variax().document.as_ref().unwrap().preset().name(),
            "Default Patch"
        );
        app.save_dialog();
        assert_eq!(
            EditorDocument::open(&populated).unwrap().preset().name(),
            "Default Patch"
        );

        let count = app.tabs.entries.len();
        app.open_file_dialog();
        assert!(app.shell_error.is_none());
        assert_eq!(app.tabs.entries.len(), count + 1);
        let active = app.tabs.index(app.tabs.active.unwrap()).unwrap();
        assert_eq!(
            app.tabs.entries[active].editor.source_path(),
            Some(collection.as_path())
        );
        assert!(
            EditorDocument::open(&collection)
                .unwrap()
                .selected_preset()
                .is_none()
        );
    }

    #[cfg(feature = "usb")]
    #[test]
    fn backup_conversion_opens_a_new_bundle_and_leaves_the_source_tree_untouched() {
        let (ctx, mut app) = app();
        let root = tempfile::tempdir().unwrap();
        let partial = synthetic_backup::make_backup(root.path(), "partial", 59, 4);
        let source = synthetic_backup::make_backup(root.path(), "backup", 60, 4);
        let before = synthetic_backup::snapshot(&source);
        let collision = root.path().join("existing.vwb");
        save_new(&collision, b"existing output").unwrap();
        let destination = root.path().join("converted");
        let inside = source.join("invalid-export.vwb");
        let inside_copy = source.join("invalid-copy.vwm");
        app.dialogs = Box::new(FakeDialogs {
            folder: [
                Some(partial),
                Some(source.clone()),
                Some(source.clone()),
                Some(source.clone()),
                Some(source.clone()),
            ]
            .into(),
            save: [
                Some(inside.clone()),
                Some(collision.clone()),
                Some(destination.clone()),
                Some(inside_copy.clone()),
                Some(destination.clone()),
            ]
            .into(),
            ..Default::default()
        });

        const COMMAND: &str = "Open backup folder as .vwb…";
        choose_file(&ctx, &mut app, COMMAND);
        assert!(app.variax().error.as_deref().unwrap().contains("59 of 60"));
        choose_file(&ctx, &mut app, COMMAND);
        assert!(
            app.variax()
                .error
                .as_deref()
                .unwrap()
                .contains("outside the Variax backup")
        );
        assert!(!inside.exists());
        choose_file(&ctx, &mut app, COMMAND);
        assert!(app.variax().error.is_some());
        assert_eq!(fs::read(collision).unwrap(), b"existing output");

        app.variax_mut()
            .change(|preset| preset.rename("Keep this edit"));
        let edited = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let output = destination.with_extension("vwb");
        choose_file(&ctx, &mut app, COMMAND);
        assert!(app.has_prompt());
        click_prompt(&ctx, &mut app, "save");
        assert!(app.has_prompt());
        assert!(!inside_copy.exists());
        assert!(!output.exists());
        click_prompt(&ctx, &mut app, "cancel");
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), edited);

        choose_file(&ctx, &mut app, COMMAND);
        click_prompt(&ctx, &mut app, "discard");
        assert!(!app.has_prompt());
        assert!(app.variax().error.is_none());
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.source, output);
        assert_eq!(document.banks().unwrap().len(), 12);
        assert_eq!(document.as_bytes(), fs::read(&output).unwrap());
        assert!(app.variax().status.contains("2/12 captured bank names"));
        assert!(app.variax().status.contains("live knob"));
        assert!(!app.variax().dirty());
        assert_eq!(synthetic_backup::snapshot(&source), before);
    }

    #[test]
    fn rename_bank_dialog_rejects_invalid_text_and_accepts_an_undoable_name() {
        fn replace_text(ctx: &egui::Context, app: &mut Workbench, name: &str) {
            click(ctx, app, "bank name");
            frame(
                ctx,
                app,
                vec![
                    egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    },
                    egui::Event::Text(name.into()),
                ],
            );
        }

        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        click(&ctx, &mut app, "Rename bank…");
        replace_text(&ctx, &mut app, "Too long 1234567890");
        click_prompt(&ctx, &mut app, "confirm-bank-rename");
        assert!(app.has_prompt());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);

        replace_text(&ctx, &mut app, "123456789012345678");
        click_prompt(&ctx, &mut app, "confirm-bank-rename");
        assert!(!app.has_prompt());
        let doc = app.variax().document.as_ref().unwrap();
        assert_eq!(doc.selected_bank().unwrap().1.name, "123456789012345678");
        assert!(doc.is_dirty());
        assert!(!doc.slot_is_dirty(0, 1));
        let edited = doc.as_bytes().to_vec();
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
        app.variax_mut().redo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), edited);
    }
}

use super::{EditorDefaults, EditorServices, Path, PathBuf, PickRequest, VariaxEditor, egui};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, fs, io::Write};
use variax_workbench::preset::legacy_text_issue;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    default_author: String,
    artwork: Option<OsString>,
    body_definitions: Option<OsString>,
}

fn settings_path(editor_preferences: &Path) -> PathBuf {
    editor_preferences.with_file_name("variax-preferences.json")
}

fn load(path: &Path) -> Result<Preferences, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preferences::default());
        }
        Err(error) => return Err(format!("Could not read Variax preferences: {error}")),
    };
    let preferences: Preferences = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Could not read Variax preferences: {error}"))?;
    if let Some(error) = legacy_text_issue(&preferences.default_author, false) {
        return Err(format!("Invalid Variax default author. {error}"));
    }
    Ok(preferences)
}

fn update(path: &Path, change: impl FnOnce(&mut Preferences)) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    // Lock a separate file so replacing the settings file cannot release
    // another window's lock halfway through a read-modify-write.
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|error| error.to_string())?;
    lock.try_lock().map_err(|error| error.to_string())?;
    let mut preferences = load(path)?;
    change(&mut preferences);
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut temporary, &preferences)
        .map_err(|error| error.to_string())?;
    temporary
        .write_all(b"\n")
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

pub(super) fn startup_artwork(defaults: &EditorDefaults) -> (PathBuf, Option<String>) {
    let Some(path) = defaults.preferences.as_deref() else {
        return (defaults.artwork.clone(), None);
    };
    match load(&settings_path(path)) {
        Ok(preferences) => {
            // The shell supplies its resolved startup directory. A custom
            // command-line path takes precedence over the remembered folder.
            let artwork_override = std::env::args_os().skip(1).any(|arg| arg == "--artwork");
            let matches_data_root = defaults.artwork == path.with_file_name("artwork");
            #[cfg(not(test))]
            let matches_data_root =
                matches_data_root || defaults.artwork == crate::editor::default_artwork();
            let automatic = !artwork_override && matches_data_root;
            let artwork = preferences
                .artwork
                .filter(|_| automatic)
                .map_or_else(|| defaults.artwork.clone(), PathBuf::from);
            (artwork, None)
        }
        Err(error) => (defaults.artwork.clone(), Some(error)),
    }
}

pub(super) fn startup_body_definitions(defaults: &EditorDefaults) -> Option<PathBuf> {
    let path = defaults.preferences.as_deref()?;
    load(&settings_path(path))
        .ok()?
        .body_definitions
        .map(PathBuf::from)
}

#[derive(Default)]
pub(super) struct AuthorPreference {
    value: String,
    error: Option<String>,
    focus: bool,
}

impl VariaxEditor {
    pub(super) fn remember_body_definitions(&mut self, folder: &Path) -> Result<(), String> {
        if !folder.is_dir() {
            return Err("Choose the Workbench body definitions folder.".into());
        }
        if let Some(path) = self.preferences_path.as_deref() {
            update(&settings_path(path), |prefs| {
                prefs.body_definitions = Some(folder.as_os_str().to_os_string());
            })?;
        }
        self.body_definitions = Some(folder.to_owned());
        self.error = None;
        Ok(())
    }

    fn author_setting(&self) -> Result<String, String> {
        match self.preferences_path.as_deref() {
            Some(path) => Ok(load(&settings_path(path))?.default_author),
            None => Ok(self.default_author.clone()),
        }
    }

    pub(super) fn new_model_author(&self) -> String {
        self.author_setting().unwrap_or_default()
    }

    fn save_default_author(&mut self, author: String) -> Result<(), String> {
        if let Some(error) = legacy_text_issue(&author, false) {
            return Err(error.into());
        }
        if let Some(path) = self.preferences_path.as_deref() {
            update(&settings_path(path), |prefs| {
                prefs.default_author.clone_from(&author)
            })?;
        }
        self.default_author = author;
        Ok(())
    }

    pub(super) fn choose_artwork(&mut self, folder: PathBuf) {
        if let Some(path) = self.preferences_path.as_deref()
            && let Err(error) = update(&settings_path(path), |prefs| {
                prefs.artwork = Some(folder.clone().into_os_string());
            })
        {
            self.error = Some(format!("Could not remember the artwork folder: {error}"));
            return;
        }
        self.artwork = folder;
        self.textures.clear();
        self.missing_textures.clear();
        self.error = None;
    }

    pub(super) fn preference_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Preferences…").clicked() {
            match self.author_setting() {
                Ok(value) => {
                    self.author_preference = Some(AuthorPreference {
                        value,
                        focus: true,
                        ..Default::default()
                    });
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub(super) fn preference_dialog(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        let Some(mut draft) = self.author_preference.take() else {
            return;
        };
        let response = egui::Modal::new(egui::Id::new("variax-preferences")).show(ctx, |ui| {
            ui.heading("Variax preferences");
            if ui.button("Artwork folder…").clicked() {
                self.artwork_dialog(ctx, services);
            }
            if self.preferences_path.is_some() && ui.button("Choose factory firmware…").clicked()
            {
                self.choose_factory_archive(ctx, services);
            }
            if ui
                .button("Body definitions folder…")
                .on_hover_text("Override the bundled pickup positions with an extracted Workbench 1.75 coefeng/data/bodies folder.")
                .clicked()
            {
                services.request_picker::<Self>(ctx, PickRequest::Folder, |editor, path, _| {
                    if let Some(path) = path
                        && let Err(error) = editor.remember_body_definitions(&path)
                    {
                        editor.error = Some(error);
                    }
                });
            }
            ui.label("Default author for new models");
            let field = ui.text_edit_singleline(&mut draft.value);
            if std::mem::take(&mut draft.focus) {
                field.request_focus();
            }
            if let Some(error) = &draft.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            ui.horizontal(|ui| {
                let cancel = ui.button("Cancel").clicked();
                let save = ui.button("Save").clicked();
                (cancel, save)
            })
            .inner
        });
        let (cancel, save) = response.inner;
        if save {
            match self.save_default_author(draft.value.clone()) {
                Ok(()) => {
                    self.error = None;
                    return;
                }
                Err(error) => draft.error = Some(error),
            }
        } else if cancel || response.should_close() {
            return;
        }
        self.author_preference = Some(draft);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{FakeDialogs, tests::app};

    #[test]
    fn author_and_picker_folder_round_trip_without_editing_the_document() {
        let (_, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("editor-preferences.txt");
        fs::write(&legacy_path, b"0\n").unwrap();
        let chosen = directory.path().join("artist artwork");
        fs::create_dir(&chosen).unwrap();
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

        app.variax_mut().preferences_path = Some(legacy_path.clone());
        app.variax_mut().save_default_author("Ana".into()).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            artwork: [Some(chosen.clone()), None].into(),
            ..Default::default()
        });
        app.artwork_dialog();
        app.artwork_dialog();
        assert_eq!(app.variax().artwork, chosen);
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.variax().dirty());
        assert_eq!(fs::read(&legacy_path).unwrap(), b"0\n");

        let mut defaults = EditorDefaults {
            artwork: directory.path().join("artwork"),
            preferences: Some(legacy_path),
            startup_panel: None,
        };
        let mut reopened = VariaxEditor::new(&defaults);
        assert_eq!(reopened.artwork, chosen);
        assert_eq!(reopened.new_model_author(), "Ana");
        assert!(!reopened.auto_select_controls);
        let custom = directory.path().join("startup override");
        defaults.artwork = custom.clone();
        assert_eq!(VariaxEditor::new(&defaults).artwork, custom);

        assert!(reopened.save_default_author("Ana 🎸".into()).is_err());
        assert_eq!(reopened.new_model_author(), "Ana");
        reopened.save_default_author(String::new()).unwrap();
        let reopened_again = VariaxEditor::new(&defaults);
        assert!(reopened_again.new_model_author().is_empty());
        assert_eq!(
            load(&settings_path(defaults.preferences.as_ref().unwrap()))
                .unwrap()
                .artwork,
            Some(chosen.into_os_string())
        );
    }

    #[test]
    fn unreadable_preferences_are_preserved_when_an_update_fails() {
        let (_, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("editor-preferences.txt");
        let path = settings_path(&legacy_path);
        fs::write(&path, b"{broken").unwrap();
        let editor = app.variax_mut();
        let old_artwork = editor.artwork.clone();
        editor.preferences_path = Some(legacy_path);
        assert!(editor.save_default_author("New Author".into()).is_err());
        editor.choose_artwork(directory.path().to_owned());
        assert_eq!(editor.artwork, old_artwork);
        assert_eq!(fs::read(path).unwrap(), b"{broken");
        assert!(editor.error.is_some());
    }
}

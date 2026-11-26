use super::{Path, PathBuf, PresetFile, TUNING, VariaxEditor, egui};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io};
use variax_workbench::{document::save_new, edit};

pub(super) const PITCH: u32 = 0x200000;
pub(super) const DETUNE: u32 = 0x200006;
pub(super) const MIX: u32 = 0x20000c;

pub(super) const TUNINGS: [(&str, [i32; 6]); 8] = [
    ("Standard", [0; 6]),
    ("Half step down", [-1; 6]),
    ("Whole step down", [-2; 6]),
    ("Drop D", [-2, 0, 0, 0, 0, 0]),
    ("Double Drop D", [-2, 0, 0, 0, 0, -2]),
    ("DADGAD", [-2, 0, 0, -1, -2, -2]),
    ("Open D", [-2, 0, 0, -1, 0, -2]),
    ("Open G", [-2, -2, 0, 0, 0, -2]),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct Tuning {
    shifts: [i32; 6],
    twelve_string: Option<TwelveString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct TwelveString {
    mix_bits: [u32; 6],
    detune_bits: [u32; 6],
}

impl Tuning {
    fn capture(preset: &PresetFile) -> Result<Self, String> {
        let shifts = tuning_values(preset, PITCH)?.map(|bits| bits as i32);
        let twelve_string = if has_twelve_strings(preset) {
            Some(TwelveString {
                mix_bits: tuning_values(preset, MIX)?,
                detune_bits: tuning_values(preset, DETUNE)?,
            })
        } else {
            None
        };
        Ok(Self {
            shifts,
            twelve_string,
        })
    }

    fn apply(self, preset: &mut PresetFile) -> Result<(), String> {
        set_tuning_values(preset, PITCH, 0, self.shifts.map(|shift| shift as u32))?;
        if let Some(twelve) = self.twelve_string
            && has_twelve_strings(preset)
        {
            set_tuning_values(preset, MIX, 1, twelve.mix_bits)?;
            set_tuning_values(preset, DETUNE, 1, twelve.detune_bits)?;
        }
        preset.set_enabled(TUNING, true)
    }
}

fn has_twelve_strings(preset: &PresetFile) -> bool {
    preset
        .model(edit::BODY)
        .is_some_and(|body| edit::is_twelve_string(body.id))
}

fn tuning_values(preset: &PresetFile, first: u32) -> Result<[u32; 6], String> {
    let mut values = [0; 6];
    for (id, bits) in (first..first + 6).zip(&mut values) {
        let parameter = preset
            .parameter(TUNING, id)
            .ok_or("The current model is missing a tuning parameter.")?;
        edit::domain(TUNING, id)
            .expect("tuning parameter has an edit domain")
            .validate(parameter.value_type, parameter.bits)?;
        *bits = parameter.bits;
    }
    Ok(values)
}

pub(super) fn set_tuning_values(
    preset: &mut PresetFile,
    first: u32,
    kind: u32,
    values: [u32; 6],
) -> Result<(), String> {
    for (id, bits) in (first..first + 6).zip(values) {
        preset.set_parameter(TUNING, id, kind, bits)?;
    }
    Ok(())
}

type Presets = BTreeMap<String, Tuning>;

#[derive(Default)]
struct TuningLibrary {
    revision: u64,
    presets: Presets,
}

impl TuningLibrary {
    fn open(directory: &Path) -> Result<Self, String> {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.to_string()),
        };
        let mut revision = 0;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            if let Some(number) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .and_then(|name| name.parse::<u64>().ok())
                .filter(|number| name == revision_filename(*number).as_str())
            {
                revision = revision.max(number);
            }
        }
        if revision == 0 {
            return Ok(Self::default());
        }
        let path = directory.join(revision_filename(revision));
        if !fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err(format!("{} must be a regular file.", path.display()));
        }
        let file = fs::File::open(&path).map_err(|error| error.to_string())?;
        let presets = serde_json::from_reader(io::BufReader::new(file))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Self { revision, presets })
    }

    fn publish(&mut self, directory: &Path, presets: Presets) -> Result<(), String> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Tuning library is full.")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let path = directory.join(revision_filename(revision));
        let bytes = serde_json::to_vec_pretty(&presets).map_err(|error| error.to_string())?;
        // A complete new snapshot makes replace and delete atomic too. A stale
        // editor tries the same next path and cannot clobber the newer snapshot.
        save_new(&path, &bytes)?;
        *self = Self { revision, presets };
        Ok(())
    }
}

fn revision_filename(revision: u64) -> String {
    format!("{revision}.json")
}

#[derive(Default)]
enum Prompt {
    #[default]
    Browse,
    Name {
        value: String,
        focus: bool,
    },
    Confirm {
        name: String,
        replacement: Option<Tuning>,
    },
}

pub(super) struct TuningDialog {
    directory: Option<PathBuf>,
    library: Option<TuningLibrary>,
    selected: Option<String>,
    prompt: Prompt,
    error: Option<String>,
}

impl TuningDialog {
    pub(super) fn open(preferences_path: Option<&Path>) -> Self {
        let directory = preferences_path.map(|path| path.with_file_name("tuning-presets"));
        let (library, error) = match directory.as_deref().map(TuningLibrary::open) {
            Some(Ok(library)) => (Some(library), None),
            Some(Err(error)) => (None, Some(format!("Could not open saved tunings. {error}"))),
            None => (None, None),
        };
        Self {
            directory,
            library,
            selected: None,
            prompt: Prompt::Browse,
            error,
        }
    }

    fn saved(&self) -> Option<&Presets> {
        self.library.as_ref().map(|library| &library.presets)
    }

    fn change_saved(&mut self, name: String, tuning: Option<Tuning>) {
        let (Some(directory), Some(library)) = (&self.directory, &mut self.library) else {
            return;
        };
        let mut next = library.presets.clone();
        if let Some(tuning) = tuning {
            next.insert(name.clone(), tuning);
        } else {
            next.remove(&name);
        }
        match library.publish(directory, next) {
            Ok(()) => {
                self.selected = tuning.map(|_| name);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("Could not update saved tunings. {error}"));
                self.library = TuningLibrary::open(directory).ok();
            }
        }
    }

    fn begin_save(&mut self, name: &str, preset: &PresetFile) -> Result<Prompt, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err("Enter a tuning name without control characters.".into());
        }
        if TUNINGS.iter().any(|(builtin, _)| *builtin == name) {
            return Err("That name belongs to a built-in tuning. Choose another name.".into());
        }
        let tuning = Tuning::capture(preset)?;
        if self
            .saved()
            .is_some_and(|presets| presets.contains_key(name))
        {
            Ok(Prompt::Confirm {
                name: name.into(),
                replacement: Some(tuning),
            })
        } else {
            self.change_saved(name.into(), Some(tuning));
            Ok(Prompt::Browse)
        }
    }
}

impl VariaxEditor {
    pub(super) fn tuning_presets_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.tuning_dialog.take() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("variax-tuning-presets")).show(ctx, |ui| {
            ui.set_width(345.0);
            ui.heading("Tuning Presets");
            if let Some(error) = &dialog.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            dialog.prompt = match std::mem::take(&mut dialog.prompt) {
                Prompt::Browse => return self.browse_tunings(ui, &mut dialog),
                Prompt::Name { mut value, focus } => {
                    ui.label("Tuning name");
                    let response = ui.text_edit_singleline(&mut value);
                    if focus {
                        response.request_focus();
                    }
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    #[cfg(test)]
                    self.tuning_widgets.insert("Tuning name", response.rect);
                    let (save, cancel) = ui
                        .horizontal(|ui| {
                            (
                                self.tuning_button(ui, "Save", true),
                                self.tuning_button(ui, "Cancel", true),
                            )
                        })
                        .inner;
                    if cancel {
                        dialog.error = None;
                        Prompt::Browse
                    } else if save || enter {
                        let result = self
                            .document
                            .as_ref()
                            .ok_or_else(|| "No Variax model is open.".to_owned())
                            .and_then(|document| dialog.begin_save(&value, document.preset()));
                        match result {
                            Ok(prompt) => prompt,
                            Err(error) => {
                                dialog.error = Some(error);
                                Prompt::Name {
                                    value,
                                    focus: false,
                                }
                            }
                        }
                    } else {
                        Prompt::Name {
                            value,
                            focus: false,
                        }
                    }
                }
                Prompt::Confirm { name, replacement } => {
                    let label = if replacement.is_some() {
                        "Replace"
                    } else {
                        "Delete"
                    };
                    ui.label(format!("{label} the saved tuning \"{name}\"?"));
                    let (confirm, cancel) = ui
                        .horizontal(|ui| {
                            (
                                self.tuning_button(ui, label, true),
                                self.tuning_button(ui, "Cancel", true),
                            )
                        })
                        .inner;
                    if confirm {
                        dialog.change_saved(name, replacement);
                        Prompt::Browse
                    } else if cancel {
                        Prompt::Browse
                    } else {
                        Prompt::Confirm { name, replacement }
                    }
                }
            };
            true
        });
        if modal.inner && !modal.should_close() {
            self.tuning_dialog = Some(dialog);
        }
    }

    fn browse_tunings(&mut self, ui: &mut egui::Ui, dialog: &mut TuningDialog) -> bool {
        let mut load = false;
        #[cfg(test)]
        self.tuning_preset_rows.clear();
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                let spacing = ui.spacing_mut();
                spacing.item_spacing.y = 2.0;
                spacing.button_padding.y = 2.0;
                spacing.interact_size.y = 22.0;
                for (name, twelve_string) in TUNINGS.iter().map(|(name, _)| (*name, false)).chain(
                    dialog.library.iter().flat_map(|library| {
                        library
                            .presets
                            .iter()
                            .map(|(name, tuning)| (name.as_str(), tuning.twelve_string.is_some()))
                    }),
                ) {
                    let label = if twelve_string {
                        format!("{name} (12 string)")
                    } else {
                        name.into()
                    };
                    let response =
                        ui.selectable_label(dialog.selected.as_deref() == Some(name), label);
                    #[cfg(test)]
                    self.tuning_preset_rows.insert(name.into(), response.rect);
                    if response.clicked() {
                        dialog.selected = Some(name.into());
                    }
                    load |= response.double_clicked();
                }
            });
        let selected = dialog.selected.as_deref();
        let saved_tuning = selected.and_then(|name| dialog.saved()?.get(name).copied());
        let builtin = selected.and_then(|name| {
            TUNINGS
                .iter()
                .find(|(builtin, _)| *builtin == name)
                .map(|(_, shifts)| *shifts)
        });
        ui.separator();
        if self.tuning_button(
            ui,
            "Load Selected Tuning",
            saved_tuning.is_some() || builtin.is_some(),
        ) || load
        {
            if let Some(tuning) = saved_tuning {
                self.change(|preset| tuning.apply(preset));
            } else if let Some(shifts) = builtin {
                self.set_tuning(shifts, true);
            }
        }
        if self.tuning_button(ui, "Save Current Tuning", dialog.library.is_some()) {
            dialog.error = None;
            dialog.prompt = Prompt::Name {
                value: selected
                    .filter(|_| saved_tuning.is_some())
                    .unwrap_or_default()
                    .into(),
                focus: true,
            };
        }
        if self.tuning_button(ui, "Delete Selected Tuning", saved_tuning.is_some())
            && let Some(name) = selected
        {
            dialog.error = None;
            dialog.prompt = Prompt::Confirm {
                name: name.into(),
                replacement: None,
            };
        }
        !self.tuning_button(ui, "Done", true)
    }

    fn tuning_button(&mut self, ui: &mut egui::Ui, label: &'static str, enabled: bool) -> bool {
        let response = ui.add_enabled(enabled, egui::Button::new(label));
        #[cfg(test)]
        self.tuning_widgets.insert(label, response.rect);
        response.clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Workbench;

    fn set_twelve_string_tuning(app: &mut Workbench) -> Tuning {
        let tuning = Tuning {
            shifts: [-12, -2, 3, 0, 1, 12],
            twelve_string: Some(TwelveString {
                mix_bits: [0.0_f32, 0.125, 0.275, 0.5, 0.875, 1.0].map(f32::to_bits),
                detune_bits: [-100.0_f32, -8.567, -0.0, 0.125, 56.789, 100.0].map(f32::to_bits),
            }),
        };
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .select(0, 4)
            .unwrap();
        app.variax_mut().change(|preset| {
            tuning.apply(preset)?;
            preset.set_enabled(TUNING, false)
        });
        tuning
    }

    #[test]
    fn tuning_library_reopens_exact_snapshots_and_refuses_a_stale_writer() {
        let dir = tempfile::tempdir().unwrap();
        let library_path = dir.path().join("tuning-presets");
        let mut library = TuningLibrary::open(&library_path).unwrap();
        let mut stale = TuningLibrary::open(&library_path).unwrap();
        let tuning = Tuning {
            shifts: [-12, -2, 3, 0, 1, 12],
            twelve_string: None,
        };
        let presets = Presets::from([("Drop B".into(), tuning)]);
        library.publish(&library_path, presets.clone()).unwrap();

        let first = library_path.join("1.json");
        let original = fs::read(&first).unwrap();
        assert!(stale.publish(&library_path, Presets::new()).is_err());
        assert_eq!(stale.revision, 0);
        assert_eq!(fs::read(&first).unwrap(), original);
        let mut reopened = TuningLibrary::open(&library_path).unwrap();
        assert_eq!(reopened.presets, presets);

        reopened.publish(&library_path, Presets::new()).unwrap();
        assert!(
            TuningLibrary::open(&library_path)
                .unwrap()
                .presets
                .is_empty()
        );
        assert_eq!(fs::read(&first).unwrap(), original);
    }

    #[test]
    fn corrupt_tuning_snapshot_blocks_saves_without_overwriting_it() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = dir.path().join("preferences");
        let directory = prefs.with_file_name("tuning-presets");
        fs::create_dir(&directory).unwrap();
        let damaged = directory.join("1.json");
        save_new(&damaged, b"invalid data").unwrap();
        let dialog = TuningDialog::open(Some(&prefs));
        assert!(dialog.saved().is_none());
        assert!(
            dialog
                .error
                .unwrap()
                .contains("Could not open saved tunings")
        );
        assert_eq!(fs::read(damaged).unwrap(), b"invalid data");
    }

    #[test]
    fn twelve_string_tunings_apply_shifts_to_six_strings_without_changing_hidden_values() {
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        let tuning = set_twelve_string_tuning(&mut app);
        let document = app.variax_mut().document.as_mut().unwrap();
        document.select(0, 0).unwrap();
        let mut before = document.preset().clone();
        let mut expected = before.clone();
        set_tuning_values(
            &mut expected,
            PITCH,
            0,
            tuning.shifts.map(|shift| shift as u32),
        )
        .unwrap();
        expected.set_enabled(TUNING, true).unwrap();
        tuning.apply(&mut before).unwrap();
        assert_eq!(before.as_bytes(), expected.as_bytes());
        assert_eq!(Tuning::capture(&before).unwrap().twelve_string, None);
    }

    #[test]
    fn manage_tunings_saves_reloads_replaces_and_deletes_with_confirmation() {
        fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 680.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.variax_mut().test_fretboard(ui, true));
                    app.pending_dialog(ctx);
                },
            );
        }

        fn click(ctx: &egui::Context, app: &mut Workbench, label: &str) {
            let editor = app.variax();
            let pos = editor
                .tuning_widgets
                .get(label)
                .or_else(|| editor.tuning_preset_rows.get(label))
                .unwrap_or_else(|| panic!("missing control {label}"))
                .center();
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
            frame(ctx, app, vec![button(false)]);
            frame(ctx, app, vec![]);
            frame(ctx, app, vec![]);
        }

        fn open(ctx: &egui::Context, app: &mut Workbench) {
            frame(ctx, app, vec![]);
            frame(ctx, app, vec![]);
            click(ctx, app, "Presets");
            click(ctx, app, "Manage tuning presets…");
            assert!(app.has_prompt());
        }

        let dir = tempfile::tempdir().unwrap();
        let prefs = dir.path().join("editor-preferences.txt");
        let directory = dir.path().join("tuning-presets");
        let name = "Test tuning";

        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        app.variax_mut().preferences_path = Some(prefs.clone());
        let original = set_twelve_string_tuning(&mut app);
        open(&ctx, &mut app);
        assert_eq!(app.variax().tuning_preset_rows.len(), 8);
        click(&ctx, &mut app, "Save Current Tuning");
        frame(&ctx, &mut app, vec![egui::Event::Text(name.into())]);
        click(&ctx, &mut app, "Save");
        assert_eq!(
            TuningLibrary::open(&directory).unwrap().presets[name],
            original
        );
        click(&ctx, &mut app, "Done");
        assert!(!app.has_prompt());

        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        app.variax_mut().preferences_path = Some(prefs);
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .select(0, 4)
            .unwrap();
        app.variax_mut()
            .change(|preset| preset.set_enabled(TUNING, false));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        open(&ctx, &mut app);
        click(&ctx, &mut app, name);
        assert_eq!(
            app.variax()
                .tuning_dialog
                .as_ref()
                .unwrap()
                .selected
                .as_deref(),
            Some(name)
        );
        click(&ctx, &mut app, "Load Selected Tuning");
        assert_eq!(app.variax().error, None);
        let preset = app.variax().document.as_ref().unwrap().preset();
        assert_eq!(Tuning::capture(preset).unwrap(), original);
        assert!(preset.model(TUNING).unwrap().enabled);
        click(&ctx, &mut app, "Done");
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

        open(&ctx, &mut app);
        click(&ctx, &mut app, name);
        click(&ctx, &mut app, "Save Current Tuning");
        click(&ctx, &mut app, "Save");
        click(&ctx, &mut app, "Cancel");
        assert_eq!(TuningLibrary::open(&directory).unwrap().revision, 1);
        click(&ctx, &mut app, "Save Current Tuning");
        click(&ctx, &mut app, "Save");
        click(&ctx, &mut app, "Replace");
        let replaced = TuningLibrary::open(&directory).unwrap();
        let preset = app.variax().document.as_ref().unwrap().preset();
        assert_eq!(replaced.presets[name], Tuning::capture(preset).unwrap());
        assert_ne!(replaced.presets[name], original);
        assert_eq!(replaced.revision, 2);

        click(&ctx, &mut app, "Delete Selected Tuning");
        click(&ctx, &mut app, "Cancel");
        assert_eq!(TuningLibrary::open(&directory).unwrap().revision, 2);
        click(&ctx, &mut app, "Delete Selected Tuning");
        click(&ctx, &mut app, "Delete");
        assert!(TuningLibrary::open(&directory).unwrap().presets.is_empty());
        assert_eq!(app.variax().tuning_preset_rows.len(), 8);
        assert!(directory.join("1.json").is_file());
        assert!(directory.join("2.json").is_file());
        click(&ctx, &mut app, "Done");
        assert!(!app.has_prompt());
    }
}

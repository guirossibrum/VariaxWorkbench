use super::{
    BODY, Color32, EditorDocument, FRET_MARKER, GUITAR_SELECTION, GUITAR_SURFACE, PICKUP1, PICKUP2,
    Pos2, PresetFile, Rect, SYSTEM, Sense, TUNING, Tab, TextureHandle, WIRING, Workbench, egui,
    guitar_panel,
};
use std::{collections::HashSet, path::Path, process::Command};
use variax_workbench::{bundle::BundleFile, edit::Domain, preset::PresetText};

const GREEN: Color32 = Color32::from_rgb(167, 231, 133);
const PITCH: u32 = 0x200000;
const DETUNE: u32 = 0x200006;
const MIX: u32 = 0x20000c;
const SPREAD: u32 = 0x200012;

const STRINGS: [(&str, i32); 6] = [
    ("Low E", 4),
    ("A", 9),
    ("D", 2),
    ("G", 7),
    ("B", 11),
    ("High E", 4),
];
const NOTES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];
const TUNINGS: [(&str, [i32; 6]); 8] = [
    ("Standard", [0; 6]),
    ("Half step down", [-1; 6]),
    ("Whole step down", [-2; 6]),
    ("Drop D", [-2, 0, 0, 0, 0, 0]),
    ("Double Drop D", [-2, 0, 0, 0, 0, -2]),
    ("DADGAD", [-2, 0, 0, 0, -2, -2]),
    ("Open D", [-2, 0, 0, -1, -2, -2]),
    ("Open G", [-2, -2, 0, 0, 0, -2]),
];

pub(super) fn milestones(ui: &mut egui::Ui) {
    show_source_checkout(ui);
    ui.separator();
    ui.heading("Complete · Offline Variax 600 editing");
    ui.label(
        "Open a preset, bank or collection, or try the built-in example banks. Edit pickups, \
         controls or tuning, then choose Save copy… to keep your original file.",
    );
    ui.separator();
    ui.heading("Offline validation before device writes");
    for (label, url, status) in [
        (
            "Compare complete presets with original Workbench (#6)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/6",
            "Open. Broader matching across models, wiring and tuning is pending.",
        ),
        (
            "Expand tests for custom tunings, bends and sustain (#33)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/33",
            "Open. Audio checks need reference recordings tracked in #128.",
        ),
        (
            "Review the offline results and remaining gaps (#22)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/22",
            "Open. The write review waits for validation and backups.",
        ),
        (
            "Check the original Workbench calculation reference (#5)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/5",
            "Open. A rounding difference is documented. Further research follows the first rig milestone.",
        ),
    ] {
        ui.hyperlink_to(label, url);
        ui.label(status);
        ui.add_space(4.0);
    }
    ui.label("Keep the guitar and PODxt Live unplugged. Any hardware test needs your approval.");
    ui.separator();
    ui.heading("Project status and issue closeout");
    ui.label(
        "Maintainers can use the GitHub command-line tool (gh) from a signed-in terminal to \
         push branches and open pull requests. Close issues after all work is merged and \
         required tests pass. Link the merge and test results on the issue.",
    );
    ui.label(
        "Which work statuses can be verified from this checkout and its locally saved upstream \
         history, and which require the authoritative GitHub project board?",
    );
    ui.label(
        "The README and local Git history show what was tested or merged. GitHub tracks current \
         priorities, assignments, and issue status.",
    );
    ui.horizontal(|ui| {
        ui.hyperlink_to(
            "Project board",
            "https://github.com/users/guirossibrum/projects/17",
        );
        ui.hyperlink_to(
            "All issues",
            "https://github.com/guirossibrum/VariaxWorkbench/issues",
        );
        ui.hyperlink_to(
            "Pull requests",
            "https://github.com/guirossibrum/VariaxWorkbench/pulls",
        );
    });
}

fn show_source_checkout(ui: &mut egui::Ui) {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
    ui.heading("Git repository used by this build");
    ui.label("Local folder");
    ui.monospace(checkout.display().to_string());
    if !checkout.join(".git").exists() {
        ui.label("The source checkout is unavailable on this computer.");
        return;
    }
    let status_id = ui.id().with("source-checkout-status");
    let refresh = ui.button("Refresh repository details").clicked();
    let status = ui
        .data(|data| data.get_temp::<Option<SourceCheckout>>(status_id))
        .filter(|_| !refresh)
        .unwrap_or_else(|| {
            let status = SourceCheckout::read(checkout);
            ui.data_mut(|data| data.insert_temp(status_id, status.clone()));
            status
        });
    let Some(status) = status else {
        ui.label("The source checkout status could not be read.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label("Current branch");
        ui.monospace(status.branch.as_str());
    });
    ui.label("Configured remote (origin)");
    if let Some(origin) = &status.origin {
        ui.monospace(origin.as_str());
    } else {
        ui.weak("No origin found for this folder.");
    }
    if status.commit == "(initial)" {
        ui.label("This checkout has no commits yet.");
    } else {
        ui.label("Last local commit");
        let short_commit = status.commit.get(..7).unwrap_or(&status.commit);
        ui.monospace(short_commit)
            .on_hover_text(status.commit.as_str());
    }
    let file_label = if status.changed_files == 1 {
        "file"
    } else {
        "files"
    };
    ui.label(format!("{} uncommitted {file_label}", status.changed_files));
}

#[derive(Clone)]
struct SourceCheckout {
    commit: String,
    branch: String,
    origin: Option<String>,
    changed_files: usize,
}

impl SourceCheckout {
    fn read(checkout: &Path) -> Option<Self> {
        let git = || {
            let mut command = Command::new("git");
            command.args(["--no-optional-locks", "-C"]).arg(checkout);
            command
        };
        let output = git()
            .args([
                "status",
                "--porcelain=v2",
                "--branch",
                "--no-ahead-behind",
                "--untracked-files=all",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let mut status = Self::parse_status(&String::from_utf8_lossy(&output.stdout))?;
        status.origin = git()
            .args(["config", "--local", "--get", "remote.origin.url"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| readable_origin(String::from_utf8_lossy(&output.stdout).trim()))
            .filter(|origin| !origin.is_empty());
        Some(status)
    }

    fn parse_status(status: &str) -> Option<Self> {
        let commit = status
            .lines()
            .find_map(|line| line.strip_prefix("# branch.oid "))?
            .to_owned();
        let branch = status
            .lines()
            .find_map(|line| line.strip_prefix("# branch.head "))?
            .to_owned();
        // Porcelain v2 quotes filenames containing line breaks and emits one row per change.
        let changed_files = status.lines().filter(|line| !line.starts_with('#')).count();
        Some(Self {
            commit,
            branch,
            origin: None,
            changed_files,
        })
    }
}

fn readable_origin(origin: &str) -> String {
    if let Some((scheme, rest)) = origin.split_once("://") {
        // Remote URLs can contain credentials in the user info, query, or fragment.
        let rest = rest.split(['?', '#']).next().unwrap_or(rest);
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        return format!("{scheme}://{host}{path}");
    }
    origin.to_owned()
}

impl Workbench {
    pub(super) fn open_example(&mut self) {
        match example_document() {
            Ok(document) => {
                self.document = Some(document);
                self.drag_origin = None;
                self.tab = Tab::Pickups;
                self.status = "Opened the built-in example banks.".into();
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Example banks: {error}")),
        }
    }

    pub(super) fn guitar_header(&mut self, ui: &mut egui::Ui, available: bool) {
        guitar_panel().inner_margin(9).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            self.bank_strip(ui, available);
            self.model_details(ui, available);
        });
    }

    fn bank_strip(&mut self, ui: &mut egui::Ui, available: bool) {
        #[cfg(test)]
        self.bank_slots.clear();
        let artwork_paths = self
            .document
            .as_ref()
            .and_then(|document| {
                let (bank, _) = document.selection()?;
                Some(
                    document.banks()?[bank]
                        .presets()
                        .iter()
                        .map(|preset| preset.model(BODY).and_then(|body| body_thumbnail(body.id)))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        let artwork: Vec<_> = artwork_paths
            .into_iter()
            .map(|path| path.and_then(|path| self.texture(ui.ctx(), &path)))
            .collect();
        let document = self
            .document
            .as_ref()
            .expect("guitar header requires a document");
        let example = document.source.as_os_str().is_empty();
        let suffix = if example { " (example)" } else { "" };
        let mut next = document.selection();
        ui.horizontal_wrapped(|ui| {
            if let (Some(banks), Some((bank_index, selected))) = (document.banks(), next) {
                let bank = &banks[bank_index];
                ui.add_enabled_ui(available, |ui| {
                    let dropdown = egui::ComboBox::from_id_salt("editing-bank")
                        .selected_text(format!("Editing bank: {}{suffix}", bank.name))
                        .width(if example { 204.0 } else { 174.0 })
                        .show_ui(ui, |ui| {
                            #[cfg(test)]
                            self.bank_options.clear();
                            let mut names = HashSet::new();
                            let repeated_names: HashSet<_> = banks
                                .iter()
                                .map(|bank| bank.name.as_str())
                                .filter(|name| !names.insert(*name))
                                .collect();
                            for (index, entry) in banks.iter().enumerate() {
                                let count = entry.presets().len();
                                let title = if entry.name.is_empty() {
                                    format!("Bank {}", index + 1)
                                } else if repeated_names.contains(entry.name.as_str()) {
                                    format!("{} ({})", entry.name, index + 1)
                                } else {
                                    entry.name.clone()
                                };
                                let response = ui.add_enabled(
                                    count != 0,
                                    egui::Button::selectable(
                                        index == bank_index,
                                        format!("{title}{suffix}"),
                                    ),
                                );
                                #[cfg(test)]
                                self.bank_options.insert(index, response.rect);
                                if response.clicked() {
                                    if index != bank_index {
                                        next = Some((index, selected.min(count - 1)));
                                    }
                                    ui.close();
                                }
                            }
                        });
                    #[cfg(test)]
                    {
                        self.bank_dropdown = Some(dropdown.response.rect);
                    }
                    dropdown.response.on_hover_text(if example {
                        "These are built-in examples. Use Open to edit a saved bank file."
                    } else {
                        "Choose a bank or folder from this file"
                    });
                    for (index, (preset, thumbnail)) in
                        bank.presets().iter().zip(&artwork).enumerate()
                    {
                        let active = index == selected;
                        let changed = document.slot_is_dirty(bank_index, index);
                        let response = bank_slot(
                            ui,
                            index + 1,
                            preset.name(),
                            thumbnail.as_ref(),
                            active,
                            changed,
                        );
                        #[cfg(test)]
                        self.bank_slots.insert(index, response.rect);
                        if response.clicked() && !active {
                            next = Some((bank_index, index));
                        }
                    }
                });
            } else {
                ui.strong("Individual preset");
            }
            ui.add_enabled(false, egui::Button::new("Get from guitar").small())
                .on_disabled_hover_text("Reading the guitar is locked during offline editing");
            ui.add_enabled(false, egui::Button::new("Send to guitar").small())
                .on_disabled_hover_text("Writing to the guitar is locked during offline editing");
            if document.is_dirty() {
                ui.colored_label(FRET_MARKER, "Unsaved changes");
            }
        });

        if next != document.selection()
            && let Some((bank, slot)) = next
            && let Some(document) = &mut self.document
        {
            match document.select(bank, slot) {
                Ok(()) => {
                    self.drag_origin = None;
                    self.error = None;
                    let name = &document.banks().expect("selected bank")[bank].name;
                    self.status = format!("Editing {name}{suffix}, slot {} offline.", slot + 1);
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn model_details(&mut self, ui: &mut egui::Ui, available: bool) {
        let document = self
            .document
            .as_ref()
            .expect("model details require a document");
        let editable = available && matches!(document.preset().device_id(), 0x70000 | 0x70004);
        let can_undo = editable && document.can_undo();
        let can_redo = editable && document.can_redo();
        let mut name = document.preset().name().to_owned();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([42.0, 17.0], egui::Label::new("Model"));
                    if ui
                        .add_enabled(
                            editable,
                            egui::TextEdit::singleline(&mut name)
                                .desired_width(168.0)
                                .hint_text("Model name"),
                        )
                        .changed()
                    {
                        self.change(|preset| preset.rename(&name));
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_sized([42.0, 17.0], egui::Label::new("Author"));
                    self.metadata_field(ui, editable, PresetText::Author, 168.0);
                });
            });
            ui.label("Notes");
            let notes_width = (ui.available_width() - 135.0).clamp(80.0, 400.0);
            self.metadata_field(ui, editable, PresetText::Notes, notes_width);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
            });
        });
    }

    fn metadata_field(&mut self, ui: &mut egui::Ui, editable: bool, field: PresetText, width: f32) {
        let document = self
            .document
            .as_ref()
            .expect("metadata fields require a document");
        match document.preset().metadata(field) {
            Ok(mut value) => {
                let widget = match field {
                    PresetText::Author => egui::TextEdit::singleline(&mut value),
                    PresetText::Notes => egui::TextEdit::multiline(&mut value).desired_rows(2),
                };
                if ui
                    .add_enabled(editable, widget.desired_width(width))
                    .changed()
                {
                    self.change(|preset| preset.set_metadata(field, &value));
                }
            }
            Err(error) => {
                ui.weak("Unavailable").on_hover_text(error);
            }
        }
    }

    pub(super) fn fretboard(&mut self, ui: &mut egui::Ui, can_edit: bool) {
        let tuning = self
            .document
            .as_ref()
            .and_then(|d| d.preset().model(TUNING));
        let enabled = tuning.is_some_and(|model| model.enabled);
        let can_edit = can_edit && tuning.is_some() && self.can_edit(TUNING, None);
        let complete_pitch = can_edit && (PITCH..PITCH + 6).all(|id| self.tuning_parameter(id, 0));
        let twelve_string = self.can_edit(TUNING, Some(MIX));
        let complete_mix = can_edit && (MIX..MIX + 6).all(|id| self.tuning_parameter(id, 1));
        ui.visuals_mut().selection = GUITAR_SELECTION;
        let spacing = ui.spacing_mut();
        spacing.item_spacing.y = 2.0;
        spacing.button_padding.y = 1.0;
        spacing.button_padding.x = 5.0;
        spacing.interact_size.y = 14.0;
        let toolbar = ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            let toggle = ui
                .add_enabled(
                    can_edit,
                    egui::Button::new("Enable").small().selected(enabled),
                )
                .on_hover_text(if enabled {
                    "Turn off alternate tuning"
                } else {
                    "Turn on alternate tuning"
                });
            #[cfg(test)]
            {
                self.tuning_toggle = Some(toggle.rect);
            }
            if toggle.clicked() {
                self.change(|preset| preset.set_enabled(TUNING, !enabled));
            }
            ui.add_enabled_ui(complete_pitch, |ui| {
                let menu = ui.menu_button("Presets", |ui| {
                    for (name, offsets) in TUNINGS {
                        let option = ui.button(name);
                        #[cfg(test)]
                        self.tuning_widgets.insert(name, option.rect);
                        if option.clicked() {
                            self.set_tuning(offsets, true);
                            ui.close();
                        }
                    }
                });
                #[cfg(test)]
                self.tuning_widgets.insert("Presets", menu.response.rect);
                #[cfg(not(test))]
                let _ = menu;
            });
            if !can_edit
                && self
                    .document
                    .as_ref()
                    .is_some_and(|d| d.preset().edit_availability(TUNING, None).is_err())
            {
                ui.weak("Alternate tuning unavailable for this body");
            }
        });

        ui.horizontal_top(|ui| {
            let rows = self.tuning_strings(ui, can_edit, twelve_string);
            self.draw_fretboard(
                ui,
                rows,
                can_edit,
                toolbar.response.rect.center().y,
                complete_pitch,
            );
        });
        ui.horizontal(|ui| {
            let lock = ui
                .add_enabled(
                    complete_mix,
                    egui::Checkbox::new(&mut self.link_tuning_mix, "Lock"),
                )
                .on_hover_text("Link all twelve-string mix controls")
                .on_disabled_hover_text(
                    "Mix lock requires a twelve-string body and six mix controls",
                );
            let reset = ui
                .add_enabled(complete_pitch, egui::Button::new("Reset").small())
                .on_hover_text("Return all six strings to standard tuning");
            #[cfg(test)]
            {
                self.tuning_widgets.insert("Lock", lock.rect);
                self.tuning_widgets.insert("Reset", reset.rect);
            }
            #[cfg(not(test))]
            let _ = lock;
            if reset.clicked() {
                self.set_tuning([0; 6], false);
            }
            if twelve_string {
                let detune =
                    can_edit && (DETUNE..DETUNE + 6).all(|id| self.tuning_parameter(id, 1));
                if ui
                    .add_enabled(detune, egui::Button::new("Zero detune").small())
                    .on_hover_text("Reset all twelve-string detune values to zero")
                    .clicked()
                {
                    self.change(|preset| set_tuning_values(preset, DETUNE, 1, [0; 6]));
                }
                ui.add_enabled_ui(can_edit, |ui| {
                    self.float_control(ui, TUNING, SPREAD, "Spread", "")
                });
            }
        });
    }

    fn tuning_parameter(&self, id: u32, kind: u32) -> bool {
        self.can_edit(TUNING, Some(id))
            && self
                .document
                .as_ref()
                .and_then(|document| document.preset().parameter(TUNING, id))
                .is_some_and(|value| value.value_type == kind)
    }

    fn set_tuning(&mut self, offsets: [i32; 6], enable: bool) {
        self.change(|preset| {
            set_tuning_values(preset, PITCH, 0, offsets.map(|offset| offset as u32))?;
            if enable {
                preset.set_enabled(TUNING, true)?;
            }
            Ok(())
        });
    }

    fn transpose_tuning(&mut self, shift: i32) {
        self.change(|preset| {
            for i in 0..6 {
                let id = PITCH + i;
                let current = preset
                    .parameter(TUNING, id)
                    .ok_or("Missing tuning offset")?
                    .bits as i32;
                let next = current.saturating_add(shift).clamp(-12, 12);
                preset.set_parameter(TUNING, id, 0, next as u32)?;
            }
            Ok(())
        });
    }

    fn tuning_strings(
        &mut self,
        ui: &mut egui::Ui,
        can_edit: bool,
        twelve_string: bool,
    ) -> [f32; 6] {
        let mut rows = [0.0; 6];
        egui::Grid::new("tuning-strings")
            .spacing([8.0, 1.0])
            .striped(true)
            .show(ui, |ui| {
                ui.weak("String");
                if twelve_string {
                    ui.weak("Mix");
                    ui.weak("Detune");
                }
                ui.weak("Shift");
                ui.weak("Note");
                ui.end_row();
                for (index, (label, pitch_class)) in STRINGS.into_iter().enumerate().rev() {
                    let index_id = index as u32;
                    let id = PITCH + index_id;
                    let pitch = self.integer(TUNING, id);
                    ui.label(label);
                    if twelve_string {
                        self.twelve_string_value(ui, MIX + index_id, can_edit);
                        self.twelve_string_value(ui, DETUNE + index_id, can_edit);
                    }
                    if let Some(mut value) = pitch {
                        if ui
                            .add_enabled(
                                can_edit && self.tuning_parameter(id, 0),
                                egui::DragValue::new(&mut value)
                                    .range(-12..=12)
                                    .clamp_existing_to_range(false),
                            )
                            .on_hover_text("Drag or click to set semitones")
                            .changed()
                        {
                            self.change(|preset| preset.set_parameter(TUNING, id, 0, value as u32));
                        }
                    } else {
                        ui.label("—");
                    }
                    let note = pitch.and_then(|offset| {
                        (-12..=12)
                            .contains(&offset)
                            .then(|| NOTES[(pitch_class + offset).rem_euclid(12) as usize])
                    });
                    rows[index] = ui.label(note.unwrap_or("—")).rect.center().y;
                    ui.end_row();
                }
            });
        rows
    }

    fn twelve_string_value(&mut self, ui: &mut egui::Ui, id: u32, can_edit: bool) {
        let Some(mut value) = self.float(TUNING, id).filter(|v| v.is_finite()) else {
            ui.label("—");
            return;
        };
        let Some(Domain::Float { min, max, .. }) = variax_workbench::edit::domain(TUNING, id)
        else {
            return;
        };
        let is_mix = id >= MIX;
        let linked = is_mix && self.link_tuning_mix;
        let can_edit = can_edit
            && self.tuning_parameter(id, 1)
            && (!linked || (MIX..MIX + 6).all(|id| self.tuning_parameter(id, 1)));
        let speed = if is_mix { 0.01 } else { 1.0 };
        let response = ui.add_enabled(
            can_edit,
            egui::DragValue::new(&mut value)
                .range(min..=max)
                .clamp_existing_to_range(false)
                .speed(speed),
        );
        #[cfg(test)]
        if id == MIX {
            self.tuning_widgets.insert("Low E mix", response.rect);
        }
        if response.changed() {
            self.change(|preset| {
                if linked {
                    set_tuning_values(preset, MIX, 1, [value.to_bits(); 6])
                } else {
                    preset.set_parameter(TUNING, id, 1, value.to_bits())
                }
            });
        }
    }

    fn draw_fretboard(
        &mut self,
        ui: &mut egui::Ui,
        rows: [f32; 6],
        can_edit: bool,
        toolbar_y: f32,
        complete_pitch: bool,
    ) {
        let width = ui.available_width();
        if width < 50.0 {
            return;
        }
        let (area, _) = ui.allocate_exact_size(
            egui::vec2(width, rows[0] - ui.cursor().top() + 22.0),
            Sense::hover(),
        );
        let neck = Rect::from_min_max(
            Pos2::new(area.left() + 11.0, rows[5] - 9.0),
            Pos2::new(area.right() - 11.0, rows[0] + 9.0),
        );
        let painter = ui.painter().clone();
        painter.rect_filled(
            neck.expand2(egui::vec2(7.0, 0.0)),
            4,
            Color32::from_rgb(68, 44, 32),
        );
        let fret_x = |offset: i32| neck.center().x + neck.width() * offset as f32 / 24.0;
        for offset in -12..=12 {
            let (weight, color) = if offset == 0 {
                (2.0_f32, FRET_MARKER)
            } else {
                (1.0, Color32::from_rgb(133, 106, 86))
            };
            painter.vline(
                fret_x(offset),
                neck.y_range(),
                egui::Stroke::new(weight, color),
            );
            if matches!(offset, -12 | -7 | -5 | 0 | 5 | 7 | 12) {
                painter.text(
                    Pos2::new(fret_x(offset), neck.bottom() + 4.0),
                    egui::Align2::CENTER_TOP,
                    if offset == 0 {
                        "0".into()
                    } else {
                        format!("{offset:+}")
                    },
                    egui::FontId::monospace(10.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }
        for fret in [-11_i32, -9, -7, -5, -3, 3, 5, 7, 9, 11] {
            let x = (fret_x(fret - fret.signum()) + fret_x(fret)) * 0.5;
            painter.circle_filled(
                Pos2::new(x, neck.center().y),
                2.5,
                Color32::from_rgb(184, 166, 143),
            );
        }

        let enabled = self
            .document
            .as_ref()
            .and_then(|d| d.preset().model(TUNING))
            .is_some_and(|model| model.enabled);
        for (index, y) in rows.into_iter().enumerate() {
            let id = PITCH + index as u32;
            painter.hline(
                neck.x_range().expand(5.0),
                y,
                egui::Stroke::new(1.2 - index as f32 * 0.1, Color32::from_rgb(204, 184, 160)),
            );
            if can_edit && self.tuning_parameter(id, 0) {
                let response = ui
                    .interact(
                        Rect::from_center_size(
                            Pos2::new(neck.center().x, y),
                            egui::vec2(neck.width() + 12.0, 16.0),
                        ),
                        ui.id().with(("fretboard", index)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("Click a fret to tune {}", STRINGS[index].0));
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let offset =
                        ((pointer.x - neck.center().x) * 24.0 / neck.width()).round() as i32;
                    self.change(|preset| {
                        preset.set_parameter(TUNING, id, 0, offset.clamp(-12, 12) as u32)
                    });
                }
            }
            if let Some(offset) = self
                .integer(TUNING, id)
                .filter(|offset| (-12..=12).contains(offset))
            {
                let color = if enabled {
                    FRET_MARKER
                } else {
                    Color32::from_rgb(190, 160, 132)
                };
                painter.circle(
                    Pos2::new(fret_x(offset), y),
                    4.0,
                    color,
                    egui::Stroke::new(1.0_f32, GUITAR_SURFACE),
                );
            }
        }
        self.transpose_buttons(ui, Pos2::new(fret_x(0), toolbar_y), complete_pitch);
    }

    fn transpose_buttons(&mut self, ui: &mut egui::Ui, center: Pos2, enabled: bool) {
        for (name, shift, path, description) in [
            (
                "Flat",
                -1,
                "buttons/flat.png",
                "Lower every string by one semitone",
            ),
            (
                "Sharp",
                1,
                "buttons/sharp.png",
                "Raise every string by one semitone",
            ),
        ] {
            let texture = self.texture(ui.ctx(), path);
            let button_center = center + egui::vec2(14.0 * shift as f32, 0.0);
            let rect = Rect::from_center_size(button_center, egui::Vec2::splat(24.0));
            let response = ui
                .place(rect, |ui: &mut egui::Ui| {
                    if !enabled {
                        ui.disable();
                    }
                    let (rect, response) = ui.allocate_exact_size(rect.size(), Sense::click());
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            response.enabled(),
                            name,
                        )
                    });
                    let visuals = ui.style().interact(&response);
                    if let Some(texture) = &texture {
                        let [width, height] = texture.size();
                        let frame_width = 22.0 * width as f32 / (height / 2) as f32;
                        let icon =
                            Rect::from_center_size(rect.center(), egui::vec2(frame_width, 22.0));
                        // The original button sheet stacks the released and pressed frames vertically.
                        let frame = if response.is_pointer_button_down_on() {
                            0.5
                        } else {
                            0.0
                        };
                        let uv =
                            Rect::from_min_max(Pos2::new(0.0, frame), Pos2::new(1.0, frame + 0.5));
                        ui.painter().image(texture.id(), icon, uv, Color32::WHITE);
                        if response.highlighted() || response.has_focus() || response.hovered() {
                            ui.painter()
                                .circle_stroke(rect.center(), 11.5, visuals.fg_stroke);
                        }
                    } else {
                        let painter = ui.painter();
                        painter.circle(rect.center(), 10.5, visuals.bg_fill, visuals.bg_stroke);
                        paint_accidental(painter, rect.center(), shift, visuals.text_color());
                    }
                    response
                })
                .on_hover_text(description);
            #[cfg(test)]
            self.tuning_widgets.insert(name, response.rect);
            if response.clicked() {
                self.transpose_tuning(shift);
            }
        }
    }
}

fn body_thumbnail(body_id: u32) -> Option<String> {
    // The archived editor shares some body panels but assigns distinct slot artwork.
    let thumbnails = [
        "tele",
        "tele",
        "thinline",
        "strat",
        "lespaul",
        "lespaul",
        "lespaul",
        "juniorsinglecut",
        "lpspecial",
        "firebird",
        "gretsch6120",
        "sparklejet",
        "ric6",
        "ric12",
        "es335",
        "casino",
        "es175",
        "super400",
        "martind28",
        "martind12_28",
        "martin018",
        "guildf212",
        "j200",
        "dobro",
        "sitar",
        "danelectro",
        "banjo",
        "tricone",
    ];
    let index = body_id.checked_sub(0x0310_0000)? as usize;
    thumbnails
        .get(index)
        .map(|name| format!("bodies/buttons/{name}.png"))
}

fn bank_slot(
    ui: &mut egui::Ui,
    position: usize,
    name: &str,
    thumbnail: Option<&TextureHandle>,
    active: bool,
    changed: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(43.0, 36.0), Sense::click());
    let description = if changed {
        format!("Slot {position} · {name} · differs from the last saved file")
    } else {
        format!("Slot {position} · {name}")
    };
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            active,
            &description,
        )
    });
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    let fill = if active {
        Color32::from_rgb(47, 68, 46)
    } else {
        Color32::from_rgb(44, 36, 30)
    };
    let stroke = if response.has_focus() || response.hovered() {
        visuals.fg_stroke
    } else if active {
        egui::Stroke::new(1.0_f32, GREEN)
    } else {
        visuals.bg_stroke
    };
    painter.rect(rect, 5, fill, stroke, egui::StrokeKind::Inside);
    if let Some(texture) = thumbnail {
        let icon = Rect::from_center_size(rect.center(), egui::vec2(26.88, 32.0));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let tint = if active || response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_gray(195)
        };
        painter.image(texture.id(), icon, uv, tint);
        let digits = position.ilog10() + 1;
        let badge = Rect::from_min_size(
            rect.min + egui::vec2(1.0, 1.0),
            egui::vec2(6.0 + 6.0 * digits as f32, 13.0),
        );
        painter.rect_filled(
            badge,
            3,
            if active {
                GREEN
            } else {
                Color32::from_rgb(220, 212, 198)
            },
        );
        painter.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            position,
            egui::FontId::proportional(10.0),
            GUITAR_SURFACE,
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            position,
            egui::FontId::proportional(15.0),
            if active { GREEN } else { visuals.text_color() },
        );
    }
    if changed {
        painter.text(
            rect.right_bottom() - egui::vec2(4.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            "!",
            egui::FontId::proportional(15.0),
            FRET_MARKER,
        );
    }
    response.on_hover_text(description)
}

fn paint_accidental(painter: &egui::Painter, center: Pos2, shift: i32, color: Color32) {
    let at = |x, y| center + egui::vec2(x, y);
    let stroke = egui::Stroke::new(1.35_f32, color);
    if shift < 0 {
        painter.line_segment([at(-2.0, -6.5), at(-2.0, 5.5)], stroke);
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [at(-2.0, 0.0), at(4.0, -5.0), at(5.5, 0.5), at(-2.0, 5.5)],
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
    } else {
        painter.line_segment([at(-2.0, -5.2), at(-2.0, 6.3)], stroke);
        painter.line_segment([at(2.0, -6.3), at(2.0, 5.2)], stroke);
        let bars = egui::Stroke::new(1.8_f32, color);
        painter.line_segment([at(-4.7, -0.8), at(4.7, -3.0)], bars);
        painter.line_segment([at(-4.7, 3.0), at(4.7, 0.8)], bars);
    }
}

fn set_tuning_values(
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

fn example_document() -> Result<EditorDocument, String> {
    fn chunk(output: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        output.extend(tag);
        output.extend((data.len() as u32).to_be_bytes());
        output.extend(data);
        if !data.len().is_multiple_of(2) {
            output.push(0);
        }
    }

    let mut base =
        PresetFile::parse(include_bytes!("../tests/fixtures/synthetic-host-preset.vwm").to_vec())?;
    base.select_model(PICKUP1, 0x03110004)?;
    base.select_model(PICKUP2, 0x03110003)?;
    for (slot, id, value) in [
        (PICKUP1, 0x200000, 1.8_f32),
        (PICKUP1, 0x200001, 8.7),
        (PICKUP1, 0x200003, 1.3),
        (PICKUP2, 0x200000, 3.88),
        (PICKUP2, 0x200003, 1.4),
        (WIRING, 0x200002, 470.0),
        (WIRING, 0x200003, 0.011),
        (SYSTEM, 0x200000, 0.0),
    ] {
        base.set_parameter(slot, id, 1, value.to_bits())?;
    }
    base.set_parameter(WIRING, 0x200001, 0, 1)?;
    set_tuning_values(&mut base, PITCH, 0, [0; 6])?;

    let mut bundle = b"L6BA".to_vec();
    chunk(&mut bundle, b"HEAD", &1u32.to_be_bytes());
    let bodies = [
        [
            ("T-Model example", 1),
            ("Spank example", 3),
            ("Lester example", 4),
            ("R-Billy example", 10),
            ("Twelve-string example", 13),
        ],
        [
            ("Spank neck example", 3),
            ("Lester Goldtop example", 5),
            ("T-Model Custom example", 0),
            ("Spank Drop D example", 3),
            ("Lester Custom example", 6),
        ],
    ];
    for (name, models) in ["Custom 1", "Custom 2"].into_iter().zip(bodies) {
        let mut bank = b"BANK".to_vec();
        let mut info = vec![0; 68];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        for (unit, dest) in name.encode_utf16().zip(info[4..].as_chunks_mut::<2>().0) {
            *dest = unit.to_be_bytes();
        }
        chunk(&mut bank, b"BINF", &info);
        for (model, body) in models {
            let mut preset = base.clone();
            preset.rename(model)?;
            preset.select_model(BODY, 0x03100000 + body)?;
            if body == 13 {
                set_tuning_values(&mut preset, DETUNE, 1, [0; 6])?;
                set_tuning_values(&mut preset, MIX, 1, [0.5_f32.to_bits(); 6])?;
                preset.set_parameter(TUNING, SPREAD, 1, 0)?;
            }
            if model == "Spank Drop D example" {
                set_tuning_values(&mut preset, PITCH, 0, [(-2_i32) as u32, 0, 0, 0, 0, 0])?;
                preset.set_enabled(TUNING, true)?;
            }
            bank.extend(preset.as_bytes());
        }
        chunk(&mut bundle, b"LIST", &bank);
    }
    let mut bytes = Vec::new();
    chunk(&mut bytes, b"FORM", &bundle);
    // The bundled examples have no source file.
    let mut document = EditorDocument::from_bundle(BundleFile::parse(bytes)?, Default::default())?;
    document.select(0, 1)?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_status_counts_renames_and_untracked_files_individually() {
        let status = SourceCheckout::parse_status(concat!(
            "# branch.oid e3e2fbbd1e55f493d1606a841b9e5a1acea646ff\n",
            "# branch.head master\n",
            "1 .M N... 100644 100644 100644 abcdef0 abcdef0 .gitignore\n",
            "2 R. N... 100644 100644 100644 abcdef0 abcdef0 R100 new.rs\told.rs\n",
            "? app/guitar.rs\n",
            "? app/main.rs\n",
            "? \"docs/file with a\\nline break.md\"\n",
        ))
        .unwrap();
        assert_eq!(status.commit, "e3e2fbbd1e55f493d1606a841b9e5a1acea646ff");
        assert_eq!(status.branch, "master");
        assert_eq!(status.changed_files, 5);

        let clean =
            SourceCheckout::parse_status("# branch.oid (initial)\n# branch.head main\n").unwrap();
        assert_eq!(clean.commit, "(initial)");
        assert_eq!(clean.branch, "main");
        assert_eq!(clean.changed_files, 0);
    }

    #[test]
    fn checkout_origin_omits_url_credentials_and_preserves_remote_paths() {
        assert_eq!(
            readable_origin("https://user:secret@example.com:8443/group/repo.git?token=secret#id"),
            "https://example.com:8443/group/repo.git"
        );
        assert_eq!(
            readable_origin("git@example.com:group/repo.git"),
            "git@example.com:group/repo.git"
        );
        assert_eq!(readable_origin("/tmp/repo.git"), "/tmp/repo.git");
    }

    #[test]
    fn bank_selector_labels_built_in_examples_and_preserves_file_bank_names() {
        fn contains_title(shape: &egui::Shape, expected: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text() == expected,
                egui::Shape::Vec(shapes) => {
                    shapes.iter().any(|shape| contains_title(shape, expected))
                }
                _ => false,
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example.vwb");
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        app.document.as_mut().unwrap().save_copy(&path).unwrap();

        for suffix in [" (example)", ""] {
            if suffix.is_empty() {
                app.load(&path);
            }
            for (index, name) in ["Custom 1", "Custom 2"].into_iter().enumerate() {
                let document = app.document.as_mut().unwrap();
                document.select(index, 0).unwrap();
                assert_eq!(document.banks().unwrap()[index].name, name);
                let output = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.guitar_header(ui, true));
                });
                let title = format!("Editing bank: {name}{suffix}");
                assert!(
                    output
                        .shapes
                        .iter()
                        .any(|shape| contains_title(&shape.shape, &title)),
                    "expected the bank selector to display {title:?}"
                );
            }
        }
    }

    #[test]
    fn tuning_presets_transpose_reset_and_linked_mix_edit_the_current_slot() {
        fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960.0, 680.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.fretboard(ui, true))
                        .inner
                },
            );
        }

        fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }
        }

        fn click(ctx: &egui::Context, app: &mut Workbench, name: &str) {
            let pos = app.tuning_widgets[name].center();
            frame(
                ctx,
                app,
                vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
            );
            frame(ctx, app, vec![pointer(pos, false)]);
            frame(ctx, app, vec![]);
        }

        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), Some(Tab::Controls));
        ctx.style_mut(|style| style.animation_time = 0.0);
        let offsets = |app: &Workbench| {
            std::array::from_fn(|i| app.integer(TUNING, PITCH + i as u32).unwrap())
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, "Presets");
        click(&ctx, &mut app, "Drop D");
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        assert!(
            app.document
                .as_ref()
                .unwrap()
                .preset()
                .model(TUNING)
                .unwrap()
                .enabled
        );
        click(&ctx, &mut app, "Flat");
        assert_eq!(offsets(&app), [-3, -1, -1, -1, -1, -1]);
        click(&ctx, &mut app, "Sharp");
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        click(&ctx, &mut app, "Reset");
        assert_eq!(offsets(&app), [0; 6]);
        app.undo();
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        click(&ctx, &mut app, "Lock");
        assert!(!app.link_tuning_mix);

        let document = app.document.as_mut().unwrap();
        document.select(0, 4).unwrap();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, "Lock");
        assert!(app.link_tuning_mix);
        let start = app.tuning_widgets["Low E mix"].center();
        let end = start + egui::vec2(18.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)]);
        frame(&ctx, &mut app, vec![pointer(end, false)]);
        let changed = app.float(TUNING, MIX).unwrap();
        assert_ne!(changed, 0.5);
        assert!((MIX..MIX + 6).all(|id| app.float(TUNING, id) == Some(changed)));
        assert_eq!(app.tab, Tab::Controls);
    }
}
